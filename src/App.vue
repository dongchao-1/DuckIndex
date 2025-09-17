<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { Window } from '@tauri-apps/api/window';
import { TabsPaneContext } from "element-plus";
import SearchView from './components/SearchView.vue';
import SettingsView from './components/SettingsView.vue';

console.log('Tauri and Vue are ready!');

const settingsViewRef = ref(SearchView);

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
        if (settingsViewRef.value) {
          settingsViewRef.value.settingLoading = true;
        }
      } else {
        if (settingsViewRef.value) {
          settingsViewRef.value.settingLoading = false;
        }
      }

    } catch (e) {
      console.error('获取状态失败', e);
    }
    setTimeout(poll, 1000);
  }
  poll();
}

async function handleTabClick(pane: TabsPaneContext, _ev: Event) {
  console.log('Tab clicked:', pane.props.label);
  if (pane.props.label === "设置") {
    await settingsViewRef.value?.refreshIndexPathTableData();
    await settingsViewRef.value?.refreshExtensionWhitelist();
  }
}

</script>

<template>
  <div class="common-layout">
    <el-container class="full-height">

      <el-main class="flex-grow">
        <el-tabs :tab-position='"top"' class="demo-tabs" @tab-click="handleTabClick">
          <el-tab-pane label="搜索">
            <SearchView />
          </el-tab-pane>

          <el-tab-pane label="设置">
            <SettingsView ref="settingsViewRef" />
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
</style>