use anyhow::{Context, Result};
use cargo_metadata::{Metadata, MetadataCommand as MetaCmd, Package};
use globset::{Glob, GlobSetBuilder as Builder};
use std::path::{Path, PathBuf};

/// 提供关于当前工作空间的上下文信息，包括元数据和根路径。
pub struct WorkspaceContext {
    /// 工作空间的完整元数据。
    pub metadata: Metadata,
    /// 工作空间的根目录路径。
    pub root: PathBuf,
    /// 激活所有特性。
    pub all_features: bool,
    /// 激活的特性列表。
    pub features: Vec<String>,
}

impl WorkspaceContext {
    /// 从给定路径载入工作空间元数据。
    pub fn load(path: &Path) -> Result<Self> {
        let metadata = MetaCmd::new()
            .current_dir(path)
            .exec()
            .with_context(|| format!("Failed to load cargo metadata from {:?}", path))?;

        let root = metadata.workspace_root.as_std_path().to_path_buf();
        let all_features = false;
        let features = Vec::new();

        Ok(Self {
            metadata,
            root,
            all_features,
            features,
        })
    }

    /// 返回工作空间中的所有包。
    pub fn members(&self) -> Vec<&Package> {
        self.metadata.workspace_packages()
    }

    /// 根据文件路径查找所属的包。
    pub fn find_package(&self, file_path: &Path) -> Option<&Package> {
        self.members().into_iter().find(|p| {
            if let Some(parent) = p.manifest_path.parent() {
                return file_path.starts_with(parent.as_std_path());
            }
            false
        })
    }

    /// 检查指定文件在给定包的上下文中是否应被忽略。
    pub fn is_ignored(&self, package: &Package, file_path: &Path) -> bool {
        // 1. 忽略 target 目录
        if file_path.components().any(|c| c.as_os_str() == "target") {
            return true;
        }

        // 2. 检查文件名中是否包含 "generated"
        let is_gen = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|name| name.contains("generated"));
        if is_gen {
            return true;
        }

        // 3. 检查自定义忽略列表
        self.check_custom_ignore(package, file_path)
    }

    fn check_custom_ignore(&self, pkg: &Package, path: &Path) -> bool {
        let Some(ignore_list) = pkg
            .metadata
            .get("alloy-check")
            .and_then(|v| v.get("ignore"))
            .and_then(|v| v.as_array())
        else {
            return false;
        };

        let mut builder = Builder::new();
        for pattern in ignore_list {
            if let Some(glob) = pattern.as_str().and_then(|p| Glob::new(p).ok()) {
                builder.add(glob);
            }
        }

        let Ok(glob_set) = builder.build() else {
            return false;
        };

        let pkg_root = pkg
            .manifest_path
            .parent()
            .map(|p| p.as_std_path())
            .unwrap_or(Path::new(""));
        let relative_path = path.strip_prefix(pkg_root).unwrap_or(path);

        glob_set.is_match(relative_path)
    }
}
