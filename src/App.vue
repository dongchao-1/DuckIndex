<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { Window } from '@tauri-apps/api/window';
import { open } from '@tauri-apps/plugin-dialog';
import { ElMessage, TabsPaneContext } from "element-plus";
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { join } from '@tauri-apps/api/path';

console.log('Tauri and Vue are ready!');

const mainWindow = new Window('main');
console.log('Main window:', mainWindow);

// 统计状态更新
const pending = ref(0);
const running = ref(0);
const running_tasks = ref("");
const directories = ref(0);
const files = ref(0);
const items = ref(0);

onMounted(() => {
  pollStatusEverySecond();
});

function pollStatusEverySecond() {
  async function poll() {
    try {
      const status: any = await invoke('get_status', {});
      // console.log(status);
      pending.value = status.task_status_stat.pending;
      running.value = status.task_status_stat.running;
      running_tasks.value = status.task_status_stat.running_tasks.join('<br>');

      directories.value = status.index_status_stat.directories;
      files.value = status.index_status_stat.files;
      items.value = status.index_status_stat.items;

      if (status.task_status_stat.pending != 0 || status.task_status_stat.running != 0) {
        settingLoading.value = true;
      } else {
        settingLoading.value = false;
      }

    } catch (e) {
      console.error('获取状态失败', e);
    }
    setTimeout(poll, 1000);
  }
  poll();
}

// 搜索
const content = ref("");

// 搜索类型定义
interface SearchType {
  key: 'directory' | 'file' | 'item';
  title: string;
  invokeMethod: string;
  resultProcessor: (item: any) => any;
  cardTitle: (item: any) => string;
  cardMain: (item: any) => string;
  openParams: (item: any) => [string, string?];
}

// 搜索配置
const searchTypes: SearchType[] = [
  {
    key: 'directory',
    title: '目录',
    invokeMethod: 'search_directory',
    resultProcessor: (item) => ({ name: item.name, path: item.path }),
    cardTitle: (item) => item.name,
    cardMain: (item) => item.path,
    openParams: (item) => [item.path]
  },
  {
    key: 'file',
    title: '文件',
    invokeMethod: 'search_file',
    resultProcessor: (item) => ({ name: item.name, path: item.path }),
    cardTitle: (item) => item.name,
    cardMain: (item) => item.path,
    openParams: (item) => [item.path, item.name]
  },
  {
    key: 'item',
    title: '内容',
    invokeMethod: 'search_item',
    resultProcessor: async (item) => {
      const fullPath = await join(item.path, item.file);
      return { content: item.content, file: item.file, path: item.path, fullPath };
    },
    cardTitle: (item) => item.content,
    cardMain: (item) => item.fullPath,
    openParams: (item) => [item.path, item.file]
  }
];

// 统一的搜索状态管理
const searchState = ref<Record<string, { loading: boolean; results: any[] }>>({
  directory: { loading: false, results: [] },
  file: { loading: false, results: [] },
  item: { loading: false, results: [] }
});

const settingLoading = ref(false);

// 统一搜索函数
async function search() {
  // 清空所有结果
  Object.keys(searchState.value).forEach(key => {
    searchState.value[key].results = [];
  });
  
  // 并行执行所有搜索
  await Promise.all(searchTypes.map(type => performSearch(type)));
}

// 执行具体搜索
async function performSearch(searchType: SearchType) {
  const { key, invokeMethod, resultProcessor } = searchType;
  
  if (!content.value.trim()) {
    searchState.value[key].results = [];
    return;
  }
  
  searchState.value[key].loading = true;
  try {
    const offset = searchState.value[key].results.length;
    const limit = 10;
    console.log(`Searching ${key} with query:`, content.value, 'Offset:', offset, 'Limit:', limit);
    
    const results: any[] = await invoke(invokeMethod, { 
      query: content.value, 
      offset: offset, 
      limit: limit 
    });
    
    for (const item of results) {
      const processedItem = await resultProcessor(item);
      searchState.value[key].results.push(processedItem);
    }
  } finally {
    searchState.value[key].loading = false;
  }
}

// 统一的滚动处理函数
async function handleScroll(e: { scrollTop: number; scrollLeft: number }, searchType: SearchType) {
  // 使用特定的类名找到对应的滚动容器
  const scrollbar = document.querySelector(`.search-scrollbar-${searchType.key} .el-scrollbar__wrap`) as HTMLElement;
  if (!scrollbar) return;

  const scrollTop = e.scrollTop;
  const clientHeight = scrollbar.clientHeight;
  const scrollHeight = scrollbar.scrollHeight;

  if (scrollHeight - scrollTop - clientHeight < 20) {
    console.log("触发加载更多: ", searchType.key, { scrollTop, clientHeight, scrollHeight });
    if (!searchState.value[searchType.key].loading) {
      await performSearch(searchType);
    }
  }
}


// 打开目录
async function openInExplorer(path: string, file?: string) {
  try {
    // await openPath(path);
    if (file) {
      await revealItemInDir( await join(path, file));
    } else {
      await revealItemInDir(path);
    }
  } catch (error) {
    console.error('打开目录失败:', error);
    ElMessage({
      message: '打开目录失败',
      type: 'error',
    });
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
  try {
    await invoke("del_index_path", {path});
    await refreshIndexPathTableData();
  } catch (e) {
    console.error("del_index_path异常:", e);
  }
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

</script>

<template>
  <div class="common-layout">
    <el-container class="full-height">

      <el-main class="flex-grow">
        <el-tabs :tab-position='"top"' class="demo-tabs" @tab-click="handleTabClick">
          <el-tab-pane label="搜索">
            <el-input 
              v-model="content" 
              @input="search" 
              size="large" 
              :autofocus="true" 
              clearable 
              placeholder="输入需要搜索的内容" 
              class="search-input"
            />
            <el-row>
              <el-col :span="8" v-for="searchType in searchTypes" :key="searchType.key">
                <p>{{ searchType.title }}:</p>
                <el-scrollbar 
                  ref="scrollbarRef"
                  :class="['search-scrollbar', `search-scrollbar-${searchType.key}`]"
                  @scroll="(e) => handleScroll(e, searchType)"
                  style="width: 90%"
                  v-loading="searchState[searchType.key].loading"
                  element-loading-text="搜索中..."
                >
                  <el-card v-for="(item, index) in searchState[searchType.key].results" :key="item.path || item.fullPath" shadow="never">
                    <template #header>
                      <div class="card-header">
                        <span class="card-index">{{ index + 1 }}.</span>
                        <span class="card-title">{{ searchType.cardTitle(item) }}</span>
                        <el-button type="primary" class="card-action-btn" @click="openInExplorer(...searchType.openParams(item))">打开</el-button>
                      </div>
                    </template>
                    <div class="card-main">{{ searchType.cardMain(item) }}</div>
                  </el-card>
                </el-scrollbar>
              </el-col>
            </el-row>
          </el-tab-pane>

          <el-tab-pane label="设置">
            <div v-loading="settingLoading" element-loading-text="正在索引中，请稍后修改设置...">
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
            </div>
          </el-tab-pane>
        </el-tabs>
      </el-main>

      <el-footer>
        <el-row>
          <el-col :span="4">
            <el-statistic title="待索引" :value="pending" />
          </el-col>
          <el-col :span="4">
            <el-tooltip
              class="box-item"
              effect="dark"
              :content="running_tasks"
              placement="top"
              :raw-content="true"
            >
              <el-statistic title="索引中" :value="running" />
            </el-tooltip>
          </el-col>
          <el-col :span="4">
            <el-statistic title="已索引目录" :value="directories" />
          </el-col>
          <el-col :span="4">
            <el-statistic title="已索引文件" :value="files" />
          </el-col>
          <el-col :span="4">
            <el-statistic title="已索引内容" :value="items" />
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

.card-main {
  font-size: 12px;
  color: #909399;
  padding: 8px 12px;
  word-break: break-all;
  word-wrap: break-word;
  white-space: normal;
  line-height: 1.3;
}

.card-header {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  padding: 8px 12px;
}

.card-index {
  color: #909399;
  font-size: 12px;
  flex-shrink: 0;
}

.card-title {
  flex: 1;
  font-size: 14px;
  line-height: 1.3;
  word-break: break-all;
  word-wrap: break-word;
  white-space: normal;
}

.card-action-btn {
  flex-shrink: 0;
  margin-left: auto;
  padding: 4px 8px;
  font-size: 12px;
  height: 24px;
}

.search-scrollbar {
  height: calc(95vh - 250px); /* 减去header、input、footer等占用的高度 */
}

/* 重写 el-card 的默认样式 */
.search-scrollbar :deep(.el-card) {
  margin-bottom: 8px;
}

.search-scrollbar :deep(.el-card__header) {
  padding: 0;
  border-bottom: 1px solid var(--el-border-color-light);
}

.search-scrollbar :deep(.el-card__body) {
  padding: 0;
}

/* 搜索输入框居中样式 */
.search-input :deep(.el-input__inner) {
  text-align: center;
}
</style>