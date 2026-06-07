use std::io::{self, Write};

use anyhow::{Context, Result, bail};

use crate::cmd::{Query, Run};
use crate::config;
use crate::db::{Database, Epoch, Stream, StreamOptions};
use crate::error::{BrokenPipeHandler, SilentExit};
use crate::util::{self, Fzf, FzfChild};

impl Run for Query {
    fn run(&self) -> Result<()> {
        let mut db = crate::db::Database::open()?;
        self.query(&mut db).and(db.save())
    }
}

impl Query {
    fn query(&self, db: &mut Database) -> Result<()> {
        let now = util::current_time()?;

        if self.interactive {
            let mut stream = self.get_stream(db, now, false)?;
            self.query_interactive(&mut stream, now)
        } else if self.list {
            self.query_list(db, now)
        } else {
            self.query_first(db, now)
        }
    }

    fn query_interactive(&self, stream: &mut Stream, now: Epoch) -> Result<()> {
        let mut fzf = Self::get_fzf()?;
        let selection = loop {
            match stream.next() {
                Some(dir) if Some(dir.path.as_ref()) == self.exclude.as_deref() => continue,
                Some(dir) => {
                    if let Some(selection) = fzf.write(dir, now)? {
                        break selection;
                    }
                }
                None => break fzf.wait()?,
            }
        };

        if self.score {
            print!("{selection}");
        } else {
            let path = selection.get(7..).context("could not read selection from fzf")?;
            print!("{path}");
        }
        Ok(())
    }

    fn query_list(&self, db: &mut Database, now: Epoch) -> Result<()> {
        if self.list_pass(db, now, false)? {
            return Ok(());
        }
        if config::fuzzy() {
            self.list_pass(db, now, true)?;
        }
        Ok(())
    }

    /// Print every match for one pass. Returns true if anything was printed.
    fn list_pass(&self, db: &mut Database, now: Epoch, fuzzy: bool) -> Result<bool> {
        let mut stream = self.get_stream(db, now, fuzzy)?;
        let handle = &mut io::stdout().lock();
        let mut found = false;
        while let Some(dir) = stream.next() {
            if Some(dir.path.as_ref()) == self.exclude.as_deref() {
                continue;
            }
            found = true;
            let dir = if self.score { dir.display().with_score(now) } else { dir.display() };
            writeln!(handle, "{dir}").pipe_exit("stdout")?;
        }
        Ok(found)
    }

    fn query_first(&self, db: &mut Database, now: Epoch) -> Result<()> {
        if let Some(path) = self.first_match(db, now, false)? {
            return Self::print_first(path);
        }
        if config::fuzzy()
            && let Some(path) = self.first_match(db, now, true)?
        {
            return Self::print_first(path);
        }
        // 交互兜底（j 未命中→fzf）里，外层会接着进选目录，这句报错就很碍眼，
        // _ZO_QUIET=1 时静默退出（仍非零），让 shell 包装层自行决定后续
        if config::quiet() {
            bail!(SilentExit { code: 1 });
        }
        bail!("no match found");
    }

    /// Return the first non-excluded match for one pass as an owned string,
    /// dropping the stream (and its `&mut db` borrow) before returning so a
    /// second pass can run.
    fn first_match(&self, db: &mut Database, now: Epoch, fuzzy: bool) -> Result<Option<String>> {
        let mut stream = self.get_stream(db, now, fuzzy)?;
        while let Some(dir) = stream.next() {
            if Some(dir.path.as_ref()) == self.exclude.as_deref() {
                continue;
            }
            let out = if self.score {
                dir.display().with_score(now).to_string()
            } else {
                dir.display().to_string()
            };
            return Ok(Some(out));
        }
        Ok(None)
    }

    fn print_first(path: String) -> Result<()> {
        let handle = &mut io::stdout();
        writeln!(handle, "{path}").pipe_exit("stdout")
    }

    fn get_stream<'a>(&self, db: &'a mut Database, now: Epoch, fuzzy: bool) -> Result<Stream<'a>> {
        let mut options = StreamOptions::new(now)
            .with_keywords(self.keywords.iter().map(|s| s.as_str()))
            .with_exclude(config::exclude_dirs()?)
            .with_base_dir(self.base_dir.clone());
        if fuzzy {
            options = options.with_fuzzy(true, config::fuzzy_threshold()?);
        }
        if !self.all {
            let resolve_symlinks = config::resolve_symlinks();
            options = options.with_exists(true).with_resolve_symlinks(resolve_symlinks);
        }

        let stream = Stream::new(db, options);
        Ok(stream)
    }

    fn get_fzf() -> Result<FzfChild> {
        let mut fzf = Fzf::new()?;
        if let Some(fzf_opts) = config::fzf_opts() {
            fzf.env("FZF_DEFAULT_OPTS", fzf_opts)
        } else {
            fzf.args([
                // Search mode
                "--exact",
                // Search result
                "--no-sort",
                // Interface
                "--bind=ctrl-z:ignore,btab:up,tab:down",
                "--cycle",
                "--keep-right",
                // Layout
                "--border=sharp", // rounded edges don't display correctly on some terminals
                "--height=45%",
                "--info=inline",
                "--layout=reverse",
                // Display
                "--tabstop=1",
                // Scripting
                "--exit-0",
            ])
            .enable_preview()
        }
        .spawn()
    }
}
