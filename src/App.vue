<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { Window } from '@tauri-apps/api/window';
import { open } from '@tauri-apps/plugin-dialog';
import { ElMessage, ScrollbarDirection, TabsPaneContext } from "element-plus";
import { openPath, revealItemInDir } from '@tauri-apps/plugin-opener';

console.log('Tauri and Vue are ready!');

const mainWindow = new Window('main');
console.log('Main window:', mainWindow);

// 统计状态更新
const pending = ref(0);
const running = ref(0);
const failed = ref(0);

mainWindow.listen("index-task-update", ({ payload }: { event: string; payload: any }) => {
  pending.value = payload.pending;
  running.value = payload.running;
  failed.value = payload.failed;
});

// 搜索
const content = ref("");

async function search() {
  directoryResult.value = [];
  await searchDirectory();
}

// 搜索结果更新
const directoryResult = ref<any[]>([]);

async function searchDirectory() {
  if (!content.value.trim()) {
    directoryResult.value = [];
    return;
  }
  const offset = directoryResult.value.length;
  const limit = 10;
  console.log('Searching directory with query:', content.value, 'Offset:', offset, 'Limit:', limit);
  const results: any[] = await invoke("search_directory", { query: content.value, offset: offset, limit: limit });
  for (const item of results) {
    directoryResult.value.push({ name: item.name , path: item.path });
  }
}

async function directoryLoadMore(direction: ScrollbarDirection) {
  if (direction === 'bottom') {
    await searchDirectory();
  }
}

// 索引目录
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

// 打开目录
async function openDirectory(path: string) {
  try {
    // await openPath(path);
    await revealItemInDir(path);
  } catch (error) {
    console.error('打开目录失败:', error);
    ElMessage({
      message: '打开目录失败',
      type: 'error',
    });
  }
}

</script>

<template>
  <div class="common-layout">
    <el-container class="full-height">

      <el-main class="flex-grow">
        <el-tabs :tab-position='"top"' class="demo-tabs" @tab-click="handleTabClick">
          <el-tab-pane label="搜索">
              <el-input v-model="content" @input="search" size="default" placeholder="输入需要搜索的内容" />
              <el-row>
                <el-col :span="8">
                  <el-scrollbar class="search-scrollbar" @end-reached="directoryLoadMore">
                    <!-- <el-table :data="directoryResult" style="width: 100%">
                      <el-table-column type="index" />
                      <el-table-column prop="name" label="Name" />
                      <el-table-column prop="path" label="Path" />
                    </el-table> -->

                    <el-card v-for="(item, index) in directoryResult"  :key="item.path" style="max-width: 480px">
                      <template #header>
                        <div class="card-header">
                          {{ index + 1 }}. {{ item.name }}
                          <el-button type="primary" @click="openDirectory(item.path)">打开</el-button>
                        </div>
                      </template>
                      <div class="card-main">{{ item.path }}</div>
                    </el-card>
<!-- 
                    <p v-for="item in directoryResult" :key="item.path" class="scrollbar-demo-item">

                      {{ item.name }} {{ item.path }}
                    </p> -->
                  </el-scrollbar>
                </el-col>
                <el-col :span="8">
                  <div class="grid-content ep-bg-purple-light" />
                </el-col>
                <el-col :span="8">
                  <div class="grid-content ep-bg-purple" />
                </el-col>
              </el-row>
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
          <el-col :span="8">
            <el-statistic title="待索引" :value="pending" />
          </el-col>
          <el-col :span="8">
            <el-statistic title="索引中" :value="running" />
          </el-col>
          <el-col :span="8">
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

.scrollbar-demo-item {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 50px;
  margin: 10px;
  text-align: center;
  border-radius: 4px;
  background: var(--el-color-primary-light-9);
  color: var(--el-color-primary);
}

.card-main {
  font-size: 12px;
  color: #909399;
  margin-top: 4px;
}

.search-scrollbar {
  height: calc(95vh - 200px); /* 减去header、input、footer等占用的高度 */
}
</style>