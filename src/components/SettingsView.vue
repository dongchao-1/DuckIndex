<template>
  <div v-loading="settingLoading" element-loading-text="正在索引中，请稍后修改设置...">
    <el-text size="large" style="font-weight: bold;">索引路径</el-text>
    <br/>
    <el-button link type="primary" @click="handleAddIndexPathClick">增加</el-button>
    <el-table :data="tableData" style="width: 100%">
      <el-table-column prop="path" label=""/>
      <el-table-column fixed="right" label="" width="100">
        <template #default="{ row }">
          <el-button link type="primary" size="small" @click="handleDelIndexPathClick(row.path)">
            删除
          </el-button>
        </template>
      </el-table-column>
    </el-table>

    <el-divider />

    <el-text size="large" style="font-weight: bold;">索引文件类型</el-text>
    <el-tree
      ref="treeRef"
      :data="data"
      show-checkbox
      default-expand-all
      node-key="label"
      highlight-current
      :default-checked-keys="data.length > 0 ? getDefaultCheckedKeys() : []"
      :props="defaultProps"
      @check-change="handleCheckChange"
    />

  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from '@tauri-apps/plugin-dialog';
import { ElMessage } from "element-plus";

interface TableRow {
  path: string;
}

const tableData = ref<TableRow[]>([]);
const settingLoading = ref(false);
const data = ref<Tree[]>([]);

// 暴露给父组件的方法和状态
defineExpose({
  refreshIndexPathTableData,
  refreshExtensionWhitelist,
  settingLoading
});

async function refreshIndexPathTableData() {
  const index_dir_paths: string[] = await invoke("get_index_dir_paths", {});
  console.log('索引目录路径:', index_dir_paths);
  tableData.value = index_dir_paths.map(path => ({ path }));
}

async function handleDelIndexPathClick(path: string) {
  console.log('Delete index path clicked:', path);
  try {
    await invoke("del_index_path", {path});
    await refreshIndexPathTableData();
  } catch (e) {
    console.error("del_index_path异常:", e);
  }
  ElMessage({
    message: '目录删除成功',
    type: 'success',
  });
}

async function handleAddIndexPathClick() {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: '请选择一个目录',
    });

    if (selected != null) {
      console.log("Selected directory:", selected);
      try {
        const result = await invoke("add_index_path", { path: selected });
        console.log("Indexing result:", result);
        await refreshIndexPathTableData();
      } catch (e) {
        console.error("add_index_path异常:", e);
      }
      ElMessage({
        message: '目录添加成功',
        type: 'success',
      });
    }
  } catch (error) {
    console.error('打开目录选择对话框失败:', error);
  }
}

async function refreshExtensionWhitelist() {
  try {
    const extension_whitelist: [] = await invoke("get_extension_whitelist", {});
    console.log('扩展白名单:', extension_whitelist);
    
    function convertToTree(node: any): Tree {
      const tree: Tree = {
        label: node.label,
        is_extension: node.is_extension,
        checked: node.enabled || false
      };
      if (node.children && Array.isArray(node.children)) {
        tree.children = node.children.map((child: any) => convertToTree(child));
      }
      return tree;
    }
    data.value = extension_whitelist.map((item: any) => convertToTree(item));
  } catch (e) {
    console.error("get_extension_whitelist异常:", e);
    // 如果获取失败，使用默认数据
    data.value = [];
  }
}

const defaultProps = {
  children: 'children',
  label: 'label',
}

interface Tree {
  label: string
  is_extension: boolean,
  children?: Tree[]
  checked?: boolean  // 添加 checked 属性控制勾选状态
}

// 获取默认勾选的节点keys
function getDefaultCheckedKeys(): string[] {
  const checkedKeys: string[] = [];
  
  function traverse(nodes: Tree[]) {
    nodes.forEach(node => {
      if (node.checked) {
        checkedKeys.push(node.label);
      }
      if (node.children) {
        traverse(node.children);
      }
    });
  }
  
  traverse(data.value);
  return checkedKeys;
}

// 处理复选框状态改变事件
async function handleCheckChange(nodeData: Tree, checked: boolean, indeterminate: boolean) {
  console.log('节点勾选状态改变:', {
    label: nodeData.label,
    is_extension: nodeData.is_extension,
    checked: checked,
    indeterminate: indeterminate
  });

  try {
    if (nodeData.is_extension) {
      await invoke("set_extension_enabled", {
        extension: nodeData.label,
        enabled: checked
      });

      ElMessage({
        message: `${nodeData.label} 已${checked ? '启用' : '禁用'}`,
        type: 'success',
      });
    }
  } catch (error) {
    console.error('更新扩展状态失败:', error);
    ElMessage({
      message: '更新失败，请重试',
      type: 'error',
    });
    
    // 重新刷新树形控件的勾选状态
    await refreshExtensionWhitelist();
  }
}

</script>

<style scoped>
/* 设置组件的样式 */
</style>
