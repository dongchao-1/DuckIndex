<template>
  <div>
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
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { ElMessage } from "element-plus";
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { join } from '@tauri-apps/api/path';

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

const content = ref("");

// 防抖定时器
let searchDebounceTimer: number | null = null;

// 统一搜索函数
async function search() {
  if (searchDebounceTimer) {
    clearTimeout(searchDebounceTimer);
  }
  
  if (!content.value.trim()) {
    Object.keys(searchState.value).forEach(key => {
      searchState.value[key].results = [];
    });
    return;
  }
  
  // 设置新的防抖定时器，延迟执行搜索
  searchDebounceTimer = setTimeout(async () => {
    Object.keys(searchState.value).forEach(key => {
      searchState.value[key].results = [];
    });
    
    // 并行执行所有搜索
    await Promise.all(searchTypes.map(type => performSearch(type)));
  }, 500);
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
    if (file) {
      console.log("打开文件:", await join(path, file));
      await revealItemInDir(await join(path, file));
    } else {
      console.log("打开目录:", path);
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
</script>

<style scoped>
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
