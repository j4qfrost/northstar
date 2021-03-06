// Copyright (c) 2021 ESRLabs
//
//   Licensed under the Apache License, Version 2.0 (the "License");
//   you may not use this file except in compliance with the License.
//   You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
//   Unless required by applicable law or agreed to in writing, software
//   distributed under the License is distributed on an "AS IS" BASIS,
//   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//   See the License for the specific language governing permissions and
//   limitations under the License.

use anyhow::Result;
use itertools::Itertools;
use northstar::api::model::{Container, Notification, Repository, RepositoryId};
use prettytable::{format, Attr, Cell, Row, Table};
use std::{collections::HashMap, io};
use tokio::time;

pub(crate) fn notification<W: io::Write>(mut w: W, notification: &Notification) {
    // TODO
    let msg = format!("📣  {:?}", notification);
    writeln!(w, "{}", msg).ok();
}

pub(crate) fn containers<W: io::Write>(mut w: W, containers: &[Container]) -> Result<()> {
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    table.set_titles(Row::new(vec![
        Cell::new("Name").with_style(Attr::Bold),
        Cell::new("Version").with_style(Attr::Bold),
        Cell::new("Repository").with_style(Attr::Bold),
        Cell::new("Type").with_style(Attr::Bold),
        Cell::new("PID").with_style(Attr::Bold),
        Cell::new("Uptime").with_style(Attr::Bold),
    ]));
    for container in containers
        .iter()
        .sorted_by_key(|c| &c.manifest.name) // Sort by name
        .sorted_by_key(|c| c.manifest.init.is_none())
    {
        table.add_row(Row::new(vec![
            Cell::new(&container.manifest.name).with_style(Attr::Bold),
            Cell::new(&container.manifest.version.to_string()),
            Cell::new(&container.repository),
            Cell::new(
                container
                    .manifest
                    .init
                    .as_ref()
                    .map(|_| "App")
                    .unwrap_or("Resource"),
            ),
            Cell::new(
                &container
                    .process
                    .as_ref()
                    .map(|p| p.pid.to_string())
                    .unwrap_or_default(),
            )
            .with_style(Attr::ForegroundColor(prettytable::color::GREEN)),
            Cell::new(
                &container
                    .process
                    .as_ref()
                    .map(|p| format!("{:?}", time::Duration::from_nanos(p.uptime)))
                    .unwrap_or_default(),
            ),
        ]));
    }

    print_table(&mut w, &table)
}

pub(crate) fn repositories<W: io::Write>(
    mut w: W,
    repositories: &HashMap<RepositoryId, Repository>,
) -> Result<()> {
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    table.set_titles(Row::new(vec![
        Cell::new("Name").with_style(Attr::Bold),
        Cell::new("Path").with_style(Attr::Bold),
    ]));
    for (id, repo) in repositories.iter().sorted_by_key(|(i, _)| (*i).clone())
    // Sort by name
    {
        table.add_row(Row::new(vec![
            Cell::new(&id).with_style(Attr::Bold),
            Cell::new(&repo.dir.display().to_string()),
        ]));
    }

    print_table(&mut w, &table)
}

fn print_table<W: std::io::Write>(mut w: W, table: &Table) -> Result<()> {
    for line in table.to_string().lines() {
        writeln!(w, "  {}", line)?;
    }
    w.flush()?;
    Ok(())
}
