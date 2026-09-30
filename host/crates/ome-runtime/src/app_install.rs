// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ome_adb::{AppPackage, PackageError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InstallUnit {
    Single(PathBuf),
    SplitSet(Vec<PathBuf>),
}

impl InstallUnit {
    pub(crate) fn label(&self) -> &Path {
        match self {
            Self::Single(path) => path,
            Self::SplitSet(paths) => &paths[0],
        }
    }

    pub(crate) fn open(&self) -> Result<AppPackage, PackageError> {
        match self {
            Self::Single(path) => AppPackage::open(path),
            Self::SplitSet(paths) => AppPackage::open_split_set(paths),
        }
    }
}

pub(crate) fn group_install_units(paths: Vec<PathBuf>) -> Vec<InstallUnit> {
    let mut apk_by_parent: HashMap<PathBuf, Vec<usize>> = HashMap::new();
    for (index, path) in paths.iter().enumerate() {
        if is_apk(path)
            && let Some(parent) = path.parent()
        {
            apk_by_parent
                .entry(parent.to_path_buf())
                .or_default()
                .push(index);
        }
    }

    let mut grouped_at = vec![None; paths.len()];
    let mut grouped = vec![false; paths.len()];
    for indices in apk_by_parent.values() {
        if indices.len() < 2 {
            continue;
        }
        let selected = indices
            .iter()
            .map(|index| paths[*index].clone())
            .collect::<Vec<_>>();
        if let Some(ordered) = order_split_set(selected) {
            let first = *indices.iter().min().expect("group has at least two paths");
            grouped_at[first] = Some(ordered);
            for index in indices {
                grouped[*index] = true;
            }
        }
    }

    paths
        .into_iter()
        .enumerate()
        .filter_map(|(index, path)| {
            if let Some(paths) = grouped_at[index].take() {
                Some(InstallUnit::SplitSet(paths))
            } else if grouped[index] {
                None
            } else {
                Some(InstallUnit::Single(path))
            }
        })
        .collect()
}

fn order_split_set(paths: Vec<PathBuf>) -> Option<Vec<PathBuf>> {
    let names = paths
        .iter()
        .map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(str::to_ascii_lowercase)
        })
        .collect::<Option<Vec<_>>>()?;
    let explicit_bases = names
        .iter()
        .enumerate()
        .filter_map(|(index, name)| (name == "base.apk").then_some(index))
        .collect::<Vec<_>>();
    let base_index = match explicit_bases.as_slice() {
        [index] => *index,
        [] => {
            let candidates = names
                .iter()
                .enumerate()
                .filter_map(|(index, name)| (!looks_like_split(name)).then_some(index))
                .collect::<Vec<_>>();
            match candidates.as_slice() {
                [index] => *index,
                _ => return None,
            }
        }
        _ => return None,
    };
    if names
        .iter()
        .enumerate()
        .any(|(index, name)| index != base_index && !looks_like_split(name))
    {
        return None;
    }

    let base = paths[base_index].clone();
    let mut splits = paths
        .into_iter()
        .enumerate()
        .filter_map(|(index, path)| (index != base_index).then_some(path))
        .collect::<Vec<_>>();
    splits.sort();
    Some(std::iter::once(base).chain(splits).collect())
}

fn is_apk(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("apk"))
}

fn looks_like_split(name: &str) -> bool {
    name.starts_with("split_") || name.contains("config.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_four_split_apks_with_base_first() {
        let paths = vec![
            PathBuf::from("apps/split_gpdeku.config.arm64_v8a.apk"),
            PathBuf::from("apps/split_gpdeku.apk"),
            PathBuf::from("apps/base.apk"),
            PathBuf::from("apps/split_config.arm64_v8a.apk"),
        ];
        assert_eq!(
            group_install_units(paths),
            vec![InstallUnit::SplitSet(vec![
                PathBuf::from("apps/base.apk"),
                PathBuf::from("apps/split_config.arm64_v8a.apk"),
                PathBuf::from("apps/split_gpdeku.apk"),
                PathBuf::from("apps/split_gpdeku.config.arm64_v8a.apk"),
            ])]
        );
    }

    #[test]
    fn keeps_lone_apk_as_one_unit() {
        let apk = PathBuf::from("apps/only.apk");
        assert_eq!(
            group_install_units(vec![apk.clone()]),
            vec![InstallUnit::Single(apk)]
        );
    }

    #[test]
    fn keeps_xapk_and_apk_as_two_units_in_order() {
        let archive = PathBuf::from("apps/bundle.xapk");
        let apk = PathBuf::from("apps/only.apk");
        assert_eq!(
            group_install_units(vec![archive.clone(), apk.clone()]),
            vec![InstallUnit::Single(archive), InstallUnit::Single(apk)]
        );
    }

    #[test]
    fn keeps_apks_without_a_base_as_single_units() {
        let paths = vec![
            PathBuf::from("apps/split_a.apk"),
            PathBuf::from("apps/split_b.apk"),
            PathBuf::from("apps/config.en.apk"),
            PathBuf::from("apps/config.ko.apk"),
        ];
        assert_eq!(
            group_install_units(paths.clone()),
            paths
                .into_iter()
                .map(InstallUnit::Single)
                .collect::<Vec<_>>()
        );
    }
}
