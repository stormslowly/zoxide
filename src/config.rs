use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use glob::Pattern;

use crate::db::Rank;

pub fn data_dir() -> Result<PathBuf> {
    let dir = match env::var_os("_ZO_DATA_DIR") {
        Some(path) => PathBuf::from(path),
        None => dirs::data_local_dir()
            .context("could not find data directory, please set _ZO_DATA_DIR manually")?
            .join("zoxide"),
    };

    ensure!(dir.is_absolute(), "_ZO_DATA_DIR must be an absolute path");
    Ok(dir)
}

pub fn echo() -> bool {
    env::var_os("_ZO_ECHO").is_some_and(|var| var == "1")
}

pub fn exclude_dirs() -> Result<Vec<Pattern>> {
    match env::var_os("_ZO_EXCLUDE_DIRS") {
        Some(paths) => env::split_paths(&paths)
            .map(|path| {
                let pattern = path.to_str().context("invalid unicode in _ZO_EXCLUDE_DIRS")?;
                Pattern::new(pattern)
                    .with_context(|| format!("invalid glob in _ZO_EXCLUDE_DIRS: {pattern}"))
            })
            .collect(),
        None => {
            let pattern = (|| {
                let home = dirs::home_dir()?;
                let home = Pattern::escape(home.to_str()?);
                Pattern::new(&home).ok()
            })();
            Ok(pattern.into_iter().collect())
        }
    }
}

pub fn fzf_opts() -> Option<OsString> {
    env::var_os("_ZO_FZF_OPTS")
}

pub fn maxage() -> Result<Rank> {
    env::var_os("_ZO_MAXAGE").map_or(Ok(10_000.0), |maxage| {
        let maxage = maxage.to_str().context("invalid unicode in _ZO_MAXAGE")?;
        let maxage = maxage
            .parse::<u32>()
            .with_context(|| format!("unable to parse _ZO_MAXAGE as integer: {maxage}"))?;
        Ok(maxage as Rank)
    })
}

pub fn resolve_symlinks() -> bool {
    env::var_os("_ZO_RESOLVE_SYMLINKS").is_some_and(|var| var == "1")
}

pub fn fuzzy() -> bool {
    // Enabled by default; set _ZO_FUZZY=0 to disable the fuzzy fallback.
    env::var_os("_ZO_FUZZY").is_none_or(|var| var != "0")
}

pub fn quiet() -> bool {
    // Set _ZO_QUIET=1 to exit silently on no match instead of printing an error.
    env::var_os("_ZO_QUIET").is_some_and(|var| var == "1")
}

pub fn fuzzy_threshold() -> Result<f64> {
    env::var_os("_ZO_FUZZY_THRESHOLD").map_or(Ok(0.85), |val| {
        let val = val.to_str().context("invalid unicode in _ZO_FUZZY_THRESHOLD")?;
        let val = val
            .parse::<f64>()
            .with_context(|| format!("unable to parse _ZO_FUZZY_THRESHOLD as float: {val}"))?;
        Ok(val)
    })
}
