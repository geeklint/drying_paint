/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2023 Violet Leonard */

use core::fmt::Write;

use alloc::{string::String, vec::Vec};

use crate::{
    trigger::{TriggeredWatch, Watch},
    WatchName,
};

type Location = &'static core::panic::Location<'static>;

pub(crate) struct CycleDiagnostic<'ctx, O: ?Sized> {
    persist_watches: Vec<Watch<'ctx, O>>,
    watch_edges: Vec<(*const (), *const ())>,
    edge_locations: Vec<Location>,
}

impl<'ctx, O> CycleDiagnostic<'ctx, O>
where
    O: ?Sized,
{
    pub(crate) fn new() -> Self {
        Self {
            persist_watches: Vec::new(),
            watch_edges: Vec::new(),
            edge_locations: Vec::new(),
        }
    }

    pub(crate) fn track_frame(&mut self, frame: &[TriggeredWatch<'ctx, O>]) {
        for item in frame {
            let edge = item.to_edge();
            let index = self.watch_edges.partition_point(|&e| e < edge);
            let existing = self.watch_edges.get(index).copied();
            if existing != Some(edge) {
                if existing.map(|e| e.0) != Some(edge.0) {
                    self.persist_watches.push(item.clone_watch());
                }
                self.watch_edges.insert(index, edge);
                self.edge_locations.insert(index, item.trigger_location());
            }
        }
    }

    pub(crate) fn do_panic(
        self,
        panic_msg: &str,
        mut frame: Vec<TriggeredWatch<'ctx, O>>,
    ) -> ! {
        let mut frame_debug = String::new();
        let mut cycle = self.find_cycle();
        frame.retain(TriggeredWatch::is_fresh);
        frame.sort_unstable_by_key(|item| item.order());
        frame.dedup_by_key(|item| item.order());
        let mut prev_name = None;
        let full_locations = cycle.is_empty();
        for trigger in &frame {
            write_trigger_description(
                &mut frame_debug,
                &mut prev_name,
                trigger,
                full_locations,
            );
        }
        if let [first, ..] = cycle[..] {
            writeln!(frame_debug, "\nIdentified a possible cycle:").ok();
            cycle.push(first);
            let mut iter = cycle.windows(2);
            let mut and_that = "";
            write!(frame_debug, "  The").ok();
            while let Some(&[source, target]) = iter.next() {
                write!(frame_debug, " trigger at ").ok();
                self.write_edge_location(&mut frame_debug, source, target);
                write!(frame_debug, "  {and_that}caused the").ok();
                and_that = "and that ";
            }
            writeln!(frame_debug, " first trigger").ok();
        }
        panic!(
            "{}\nThe following information may explain why:\n\n{}\n",
            panic_msg, frame_debug
        )
    }

    fn find_cycle(&self) -> Vec<*const ()> {
        let mut visited = alloc::vec::Vec::new();
        'find: for root in 0..self.watch_edges.len() {
            visited.push(self.watch_edges[root].0);
            visited.push(self.watch_edges[root].1);
            while let [.., current, target] = &mut visited[..] {
                let i = match self
                    .watch_edges
                    .binary_search(&(*target, core::ptr::null()))
                {
                    Ok(i) => i,
                    Err(i) => i,
                };
                match self.watch_edges.get(i) {
                    Some(edge) if edge.0 == *target => {
                        if let Some(i) =
                            visited.iter().position(|&e| e == edge.1)
                        {
                            visited.drain(..i);
                            break 'find;
                        }
                        visited.push(edge.1);
                        continue;
                    }
                    _ => (),
                }
                let cur_idx = self
                    .watch_edges
                    .binary_search(&(*current, *target))
                    .unwrap();
                match self.watch_edges.get(cur_idx + 1) {
                    Some(edge) if edge.0 == *current => {
                        *target = edge.1;
                    }
                    _ => {
                        visited.pop();
                    }
                }
            }
            visited.clear();
        }
        visited
    }

    fn write_edge_location(
        &self,
        output: &mut String,
        source: *const (),
        target: *const (),
    ) {
        match self.watch_edges.binary_search(&(source, target)) {
            Ok(i) => {
                write_location(output, self.edge_locations[i]);
            }
            Err(_) => {
                writeln!(output, "(unknown location)").ok();
            }
        }
    }
}

fn write_trigger_description<O: ?Sized>(
    output: &mut String,
    prev_name: &mut Option<WatchName>,
    trigger: &TriggeredWatch<'_, O>,
    full_locations: bool,
) {
    if Some(trigger.watch_name()) == *prev_name {
        write!(output, "  and because ").ok();
    } else {
        use crate::trigger::watch_name::Inner;
        match trigger.watch_name().inner {
            Inner::Name(name) => {
                writeln!(output, "The watch named '{name}'").ok();
            }
            Inner::SpawnLocation(location) => {
                write!(output, "The watch created at ").ok();
                if full_locations {
                    write_location(output, location);
                } else {
                    writeln!(output, "{location}").ok();
                }
            }
        }
        write!(output, "  was going to run because ").ok();
    }
    *prev_name = Some(trigger.watch_name());
    write!(output, "it was invoked at ").ok();
    let cursor = output.lines().last().map(str::len).unwrap_or(0);
    let location = trigger.trigger_location();
    if location.file().len().saturating_add(cursor) > 70 {
        write!(output, "\n  ").ok();
    }
    if full_locations {
        write_location(output, location);
    } else {
        writeln!(output, "{}", location).ok();
    }
}

#[cfg(feature = "std")]
fn write_location(output: &mut String, location: Location) -> Option<()> {
    use core::convert::TryFrom;
    writeln!(output, "{location}").ok()?;
    let line_no = usize::try_from(location.line().saturating_sub(1)).ok()?;
    let col_no = usize::try_from(location.column().saturating_sub(1)).ok()?;
    let file_data = std::fs::read_to_string(location.file()).ok()?;
    let line = file_data.lines().nth(line_no)?;
    let trimmed = line.trim_start();
    let trimmed_col = col_no - (line.len() - trimmed.len());
    let underline = " ".repeat(trimmed_col) + "^";
    let indent = "    ";
    writeln!(output, "\n{indent}{trimmed}\n{indent}{underline}").ok()
}

#[cfg(not(feature = "std"))]
fn write_location(output: &mut String, location: Location) -> Option<()> {
    writeln!(output, "{location}").ok()
}
