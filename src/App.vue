<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { Window } from '@tauri-apps/api/window';
import { open } from '@tauri-apps/plugin-dialog';
import { ElMessage, TabsPaneContext } from "element-plus";

console.log('Tauri and Vue are ready!');

const mainWindow = new Window('main');
console.log('Main window:', mainWindow);

interface IndexTaskStatus {
  pending: number;
  running: number;
  failed: number;
  running_tasks: string[];
  failed_tasks: string[];
}

const pending = ref(0);
const running = ref(0);
const failed = ref(0);

// 监听状态更新
mainWindow.listen("index-task-update", ({ payload }: { event: string; payload: IndexTaskStatus }) => {
  pending.value = payload.pending;
  running.value = payload.running;
  failed.value = payload.failed;
});

const greetMsg = ref("");
const name = ref("");

async function search() {
  greetMsg.value = await invoke("search", { query: name.value });
}

interface TableRow {
  path: string;
}

const tableData = ref<TableRow[]>([])

async function refreshIndexPathTableData() {
  const index_dir_paths: string[] = await invoke("get_index_dir_paths", {});
  console.log('索引目录路径:', index_dir_paths);
  tableData.value = index_dir_paths.map(path => ({ path }));
}

async function handleTabClick(pane: TabsPaneContext, _ev: Event) {
  console.log('Tab clicked:', pane.props.label);
  if (pane.props.label === "设置") {
    await refreshIndexPathTableData();
  }
}

async function handleDelIndexPathClick(path: string) {
  console.log('Delete index path clicked:', path);
  await invoke("del_index_path", {path});
  await refreshIndexPathTableData();
  ElMessage({
    message: '目录删除成功',
    type: 'success',
  })
}

async function handleAddIndexPathClick() {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: '请选择一个目录',
      // defaultPath: await homeDir(),
    });

    if (selected != null) {
      console.log("Selected directory:", selected);
      const result = await invoke("add_index_path", { path: selected });
      console.log("Indexing result:", result);
      await refreshIndexPathTableData();
      ElMessage({
        message: '目录添加成功',
        type: 'success',
      })
    }
  } catch (error) {
    console.error('打开目录选择对话框失败:', error);
  }
}

</script>

<template>
  <div class="common-layout">
    <el-container class="full-height">

      <el-main class="flex-grow">
        <el-tabs :tab-position='"top"' class="demo-tabs" @tab-click="handleTabClick">
          <el-tab-pane label="搜索">
              <el-input v-model="name" @input="search" size="default" placeholder="输入需要搜索的内容" />
              <p>{{ greetMsg }}</p>
          </el-tab-pane>
          <el-tab-pane label="设置">
            <el-text class="mx-1">索引路径</el-text>
            <el-button type="primary" @click="handleAddIndexPathClick">增加</el-button>
            <el-table :data="tableData" style="width: 100%">
              <el-table-column prop="path" label="路径"/>
              <el-table-column fixed="right" label="操作" width="100">
                <template #default="{ row }">
                  <el-button link type="primary" size="small" @click="handleDelIndexPathClick(row.path)">
                    删除
                  </el-button>
                </template>
              </el-table-column>
            </el-table>
          </el-tab-pane>
        </el-tabs>
      </el-main>

      <el-footer>
        <el-row>
          <el-col :span="6">
            <el-statistic title="待索引" :value="pending" />
          </el-col>
          <el-col :span="6">
            <el-statistic title="索引中" :value="running" />
          </el-col>
          <el-col :span="6">
            <el-statistic title="索引失败" :value="failed" />
          </el-col>
        </el-row>
      </el-footer>

   </el-container>
  </div>
</template>

<style scoped>
.common-layout {
  height: 95vh; /* 确保整个容器有高度 */
}

.full-height {
  height: 100%; /* 让 el-container 撑满父容器 */
}

.flex-grow {
  flex: 1;
}
</style>