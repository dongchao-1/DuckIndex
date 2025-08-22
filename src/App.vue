<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { Window } from '@tauri-apps/api/window';
import { open } from '@tauri-apps/plugin-dialog';
import { TabsPaneContext } from "element-plus";

console.log('Tauri and Vue are ready!');

const mainWindow = new Window('main');
console.log('Main window:', mainWindow);

mainWindow.listen("index-task-update", ({ event, payload }: { event: string; payload: unknown }) => { 
  // console.log('收到索引任务更新:', event, payload);
});

const greetMsg = ref("");
const name = ref("");

async function search() {
  greetMsg.value = await invoke("search", { query: name.value });
}

async function index_all_files() {
  greetMsg.value = await invoke("index_all_files", {});
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
    }
  } catch (error) {
    console.error('打开目录选择对话框失败:', error);
  }
}

</script>

<template>
  <main class="container">
    <!-- <h1>Welcome to Tauri + Vue</h1>

    <div class="row">
      <a href="https://vite.dev" target="_blank">
        <img src="/vite.svg" class="logo vite" alt="Vite logo" />
      </a>
      <a href="https://tauri.app" target="_blank">
        <img src="/tauri.svg" class="logo tauri" alt="Tauri logo" />
      </a>
      <a href="https://vuejs.org/" target="_blank">
        <img src="./assets/vue.svg" class="logo vue" alt="Vue logo" />
      </a>
    </div>
    <p>Click on the Tauri, Vite, and Vue logos to learn more.</p> -->

    

    <!-- <form class="row" @submit.prevent="index_all_files">
      <button type="submit">重建所有索引</button>
    </form>

    
    <form class="row" @submit.prevent="search">
      <input id="search-input" v-model="name" placeholder="Enter a text..." />
      <button type="submit">搜索</button>
    </form>

    -->

    <div class="mb-4">

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
    </div>

  </main>
</template>

<style scoped>
</style>