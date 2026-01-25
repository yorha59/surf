//! TUI 目录树数据模型
//!
//! 提供从扁平文件列表构建目录树的功能，支持按大小聚合目录。

use std::path::{Path, PathBuf};
use surf_core::FileEntry;

/// 节点类型：文件或目录
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeType {
    File,
    Directory,
}

/// 目录树节点
#[derive(Debug, Clone)]
pub struct DirNode {
    /// 节点名称（路径的最后一段）
    pub name: String,
    /// 完整路径
    pub full_path: PathBuf,
    /// 节点类型
    pub node_type: NodeType,
    /// 节点大小：对于文件是文件大小，对于目录是子孙文件总大小
    pub size: u64,
    /// 子节点列表，按 size 降序排序
    pub children: Vec<DirNode>,
}

impl DirNode {
    /// 创建一个新的文件节点
    pub fn new_file(path: PathBuf, size: u64) -> Self {
        let name = path
            .file_name()
            .and_then(|os_str| os_str.to_str())
            .unwrap_or("")
            .to_string();
        DirNode {
            name,
            full_path: path,
            node_type: NodeType::File,
            size,
            children: Vec::new(),
        }
    }

    /// 创建一个新的目录节点
    pub fn new_directory(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .and_then(|os_str| os_str.to_str())
            .unwrap_or("")
            .to_string();
        DirNode {
            name,
            full_path: path,
            node_type: NodeType::Directory,
            size: 0,
            children: Vec::new(),
        }
    }

    /// 向目录节点添加子节点，并更新目录大小
    /// 注意：此方法假设子节点已经包含其正确的大小（对于目录，其大小已经聚合）
    /// 调用者需要确保子节点按正确顺序添加（通常按 size 降序）
    pub fn add_child(&mut self, child: DirNode) {
        self.size += child.size;
        self.children.push(child);
    }

    /// 对子节点按 size 降序排序
    pub fn sort_children(&mut self) {
        self.children.sort_by(|a, b| b.size.cmp(&a.size));
    }

    /// 获取当前节点的直接子节点（用于列表视图）
    pub fn children(&self) -> &[DirNode] {
        &self.children
    }

    /// 判断节点是否为文件
    pub fn is_file(&self) -> bool {
        self.node_type == NodeType::File
    }

    /// 判断节点是否为目录
    pub fn is_directory(&self) -> bool {
        self.node_type == NodeType::Directory
    }
}

/// 从扁平文件列表构建目录树（当前实现为**根目录一级聚合视图**）。
///
/// - `root_path`：扫描根目录。
/// - `entries`：来自 `surf-core` 的文件条目列表（仅文件，不含目录）。
///
/// 返回的 `DirNode`：
/// - `full_path == root_path`，`children` 包含：
///   - 直接位于根目录下的文件节点；
///   - 每个直接子目录对应一个目录节点，其 `size` 为该目录下所有子孙文件大小之和；
/// - 根节点的 `size` 为所有文件大小之和；
/// - 同一层级下子节点按 `size` 降序排序。
pub fn build_tree(root_path: &Path, entries: Vec<FileEntry>) -> DirNode {
    use std::collections::HashMap;

    // 根节点：代表当前扫描根目录。
    let mut root = DirNode::new_directory(root_path.to_path_buf());

    // 统计：
    // - 根目录下的直接文件列表；
    // - 直接子目录的聚合大小（包含其所有子孙文件）。
    let mut direct_files: Vec<DirNode> = Vec::new();
    let mut direct_dir_sizes: HashMap<PathBuf, u64> = HashMap::new();

    for entry in &entries {
        let file_path = &entry.path;
        let file_size = entry.size;

        // 累加到根目录总大小。
        root.size = root.size.saturating_add(file_size);

        // 若文件直接位于根目录下，则记录为直接文件节点。
        if file_path.parent() == Some(root_path) {
            direct_files.push(DirNode::new_file(file_path.clone(), file_size));
        }

        // 将文件大小累加到其所有祖先目录，用于计算各直接子目录的聚合大小。
        let mut current = file_path.parent();
        while let Some(dir) = current {
            if dir == root_path {
                break;
            }
            *direct_dir_sizes.entry(dir.to_path_buf()).or_insert(0) += file_size;
            current = dir.parent();
        }
    }

    // 先添加直接文件子节点。
    for file_node in direct_files {
        root.add_child(file_node);
    }

    // 为每个直接子目录构建目录节点，并附上聚合大小。
    for (dir_path, total_size) in direct_dir_sizes {
        if dir_path.parent() == Some(root_path) {
            let mut dir_node = DirNode::new_directory(dir_path);
            dir_node.size = total_size;
            root.add_child(dir_node);
        }
    }

    // 根目录下的所有子节点按 size 降序排序，用于 TUI 浏览 TopN。
    root.sort_children();

    root
}

/// 获取当前选中节点的完整路径显示
pub fn get_node_display_path(node: &DirNode) -> String {
    node.full_path.display().to_string()
}

/// 获取当前选中节点的大小显示
pub fn get_node_display_size(node: &DirNode) -> String {
    format!("{} 字节", node.size)
}

/// 获取当前选中节点的类型显示
pub fn get_node_display_type(node: &DirNode) -> String {
    match node.node_type {
        NodeType::File => "文件".to_string(),
        NodeType::Directory => "目录".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_tree_aggregates_root_and_direct_children() {
        let root = PathBuf::from("/root");

        let entries = vec![
            FileEntry { path: root.join("a.bin"), size: 10 },
            FileEntry { path: root.join("sub1").join("b.bin"), size: 20 },
            FileEntry { path: root.join("sub1").join("deep").join("c.bin"), size: 30 },
        ];

        let tree = build_tree(&root, entries);

        // 根大小为所有文件大小之和。
        assert_eq!(tree.size, 10 + 20 + 30);

        // 子节点应包含 1 个直接文件和 1 个目录（sub1）。
        assert_eq!(tree.children.len(), 2);

        let mut has_root_file = false;
        let mut has_subdir = false;

        for child in &tree.children {
            if child.is_file() && child.name == "a.bin" {
                has_root_file = true;
                assert_eq!(child.size, 10);
            }
            if child.is_directory() && child.name == "sub1" {
                has_subdir = true;
                // sub1 聚合了其下所有文件：20 + 30
                assert_eq!(child.size, 20 + 30);
            }
        }

        assert!(has_root_file, "root should expose direct file child");
        assert!(has_subdir, "root should expose direct sub-directory child");
    }
}
