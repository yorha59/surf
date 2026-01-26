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

    /// 按索引移除一个子节点，并从当前目录大小中扣除该子节点的聚合大小。
    ///
    /// - 若索引越界，返回 `None`，不修改任何状态；
    /// - 若索引合法，返回被移除的子节点，并使用 `saturating_sub` 防御性更新目录大小。
    pub fn remove_child_at(&mut self, index: usize) -> Option<DirNode> {
        if index >= self.children.len() {
            return None;
        }
        let child = self.children.remove(index);
        self.size = self.size.saturating_sub(child.size);
        Some(child)
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

/// 从扁平文件列表构建目录树。
///
/// - `root_path`：扫描根目录；
/// - `entries`：来自 `surf-core` 的文件条目列表（仅文件，不含目录）。
///
/// 返回的 `DirNode` 满足：
/// - `full_path == root_path`，表示扫描根目录；
/// - `children` 递归包含根目录下所有层级的目录和文件：
///   - 每个目录节点的 `size` 为其所有子孙文件大小之和；
///   - 根节点的 `size` 为整个扫描结果中所有文件大小之和；
/// - 同一层级下子节点按 `size` 降序排序，便于在 TUI 中按占用大小浏览。
pub fn build_tree(root_path: &Path, entries: Vec<FileEntry>) -> DirNode {
    let mut root = DirNode::new_directory(root_path.to_path_buf());

    for entry in entries {
        insert_file_into_tree(&mut root, root_path, &entry.path, entry.size);
    }

    sort_tree_by_size(&mut root);

    root
}

/// 将单个文件条目插入到目录树中，并沿途累加各级目录的聚合大小。
fn insert_file_into_tree(root: &mut DirNode, root_path: &Path, file_path: &Path, size: u64) {
    // 更新根节点聚合大小。
    root.size = root.size.saturating_add(size);

    // 若文件路径不在 root_path 之下，退化为“直接挂到根目录”。
    let mut current_dir = file_path.parent();
    if current_dir.is_none() {
        // 没有父目录，作为根节点直接子文件处理。
        root.children.push(DirNode::new_file(file_path.to_path_buf(), size));
        return;
    }

    // 收集 root_path 之下的所有祖先目录路径（不包含 root_path 本身），
    // 例如：/root/sub1/deep/file.bin -> [/root/sub1, /root/sub1/deep]
    let mut ancestors: Vec<PathBuf> = Vec::new();
    while let Some(dir) = current_dir {
        if dir == root_path {
            break;
        }
        if !dir.starts_with(root_path) {
            // 文件不在 root_path 之下，直接作为根目录子文件处理。
            ancestors.clear();
            break;
        }
        ancestors.push(dir.to_path_buf());
        current_dir = dir.parent();
    }
    ancestors.reverse();

    // 从根开始，依次下钻/创建每一级目录节点，并累加 size。
    let mut node = root;
    for dir_path in &ancestors {
        // 先通过不可变迭代找到目标子目录的索引，避免同时持有
        // `node.children` 的可变和不可变借用。
        let existing_idx = node
            .children
            .iter()
            .position(|child| child.is_directory() && child.full_path == *dir_path);

        if let Some(idx) = existing_idx {
            let existing = &mut node.children[idx];
            existing.size = existing.size.saturating_add(size);
            node = existing;
        } else {
            let mut new_dir = DirNode::new_directory(dir_path.clone());
            new_dir.size = size;
            node.children.push(new_dir);
            let len = node.children.len();
            node = &mut node.children[len - 1];
        }
    }

    // 在最终目录节点下添加文件节点；目录大小已在上面的循环中累加。
    node.children.push(DirNode::new_file(file_path.to_path_buf(), size));
}

/// 递归按 size 对整个目录树的子节点进行降序排序。
fn sort_tree_by_size(node: &mut DirNode) {
    node.sort_children();
    for child in &mut node.children {
        if child.is_directory() {
            sort_tree_by_size(child);
        }
    }
}

/// 递归重新计算整棵目录树的聚合大小。
///
/// - 对于文件节点：保留其原有 `size`，并将该值作为返回值；
/// - 对于目录节点：先递归处理所有子节点，将子节点 size 之和写回当前目录的
///   `size` 字段，并返回该聚合值。
///
/// 该函数用于在 TUI 中执行删除操作后，对整棵目录树的聚合大小进行一次性校正，
/// 确保根节点以及所有祖先目录显示的大小与当前剩余文件列表一致。
pub fn recompute_aggregated_sizes(node: &mut DirNode) -> u64 {
    if node.is_directory() {
        let mut total = 0u64;
        for child in &mut node.children {
            total = total.saturating_add(recompute_aggregated_sizes(child));
        }
        node.size = total;
        total
    } else {
        node.size
    }
}

/// 在目录树中按完整路径查找节点（只读）。
pub fn find_node<'a>(root: &'a DirNode, target: &Path) -> Option<&'a DirNode> {
    if root.full_path == target {
        return Some(root);
    }

    if !root.is_directory() {
        return None;
    }

    for child in &root.children {
        if let Some(found) = find_node(child, target) {
            return Some(found);
        }
    }

    None
}

/// 在目录树中按完整路径查找节点（可变）。
pub fn find_node_mut<'a>(root: &'a mut DirNode, target: &Path) -> Option<&'a mut DirNode> {
    if root.full_path == target {
        return Some(root);
    }

    if !root.is_directory() {
        return None;
    }

    for child in &mut root.children {
        if let Some(found) = find_node_mut(child, target) {
            return Some(found);
        }
    }

    None
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

        // 验证子目录层级结构：sub1 应该包含 deep 子目录。
        let sub1 = tree
            .children
            .iter()
            .find(|c| c.is_directory() && c.name == "sub1")
            .expect("sub1 directory should exist");

        let deep = sub1
            .children
            .iter()
            .find(|c| c.is_directory() && c.name == "deep")
            .expect("deep directory should exist under sub1");

        // deep 目录只包含一个文件 c.bin，大小为 30。
        assert_eq!(deep.size, 30);
        assert_eq!(deep.children.len(), 1);
        assert!(deep.children[0].is_file());
        assert_eq!(deep.children[0].name, "c.bin");
    }

    #[test]
    fn recompute_sizes_after_removing_child_updates_ancestors() {
        let root = PathBuf::from("/root");

        let entries = vec![
            FileEntry { path: root.join("a.bin"), size: 10 },
            FileEntry { path: root.join("sub1").join("b.bin"), size: 20 },
        ];

        let mut tree = build_tree(&root, entries);

        // 初始聚合大小：根为 30，sub1 为 20。
        assert_eq!(tree.size, 30);
        let sub1 = tree
            .children
            .iter()
            .find(|c| c.is_directory() && c.name == "sub1")
            .expect("sub1 directory should exist");
        assert_eq!(sub1.size, 20);

        // 模拟在 TUI 中删除 /root/sub1/b.bin：从 sub1 子节点列表中移除该文件。
        {
            let sub1_mut = tree
                .children
                .iter_mut()
                .find(|c| c.is_directory() && c.name == "sub1")
                .expect("sub1 directory should exist (mutable)");
            let idx = sub1_mut
                .children
                .iter()
                .position(|c| c.is_file() && c.name == "b.bin")
                .expect("b.bin should exist under sub1");
            sub1_mut.remove_child_at(idx);
        }

        // 调用聚合大小重算函数，确保根和子目录的 size 与当前剩余文件一致。
        let total_after = recompute_aggregated_sizes(&mut tree);
        assert_eq!(total_after, tree.size);
        assert_eq!(tree.size, 10);

        let sub1_after = tree
            .children
            .iter()
            .find(|c| c.is_directory() && c.name == "sub1")
            .expect("sub1 directory should still exist after delete");
        // sub1 下已无文件，聚合大小应为 0。
        assert_eq!(sub1_after.size, 0);
    }
}
