<script setup lang="ts">
import { computed } from 'vue'
import { useData, withBase } from 'vitepress'

const { frontmatter, page } = useData()
const isHome = computed(() => frontmatter.value.layout === 'home')
const isChinese = computed(() => page.value.relativePath.startsWith('zh/'))
</script>

<template>
  <nav v-if="isHome" class="umc-lang-switch" aria-label="Language">
    <a :href="withBase('/')" :class="{ active: !isChinese }" hreflang="en">EN</a>
    <a :href="withBase('/zh/')" :class="{ active: isChinese }" hreflang="zh-CN">中文</a>
  </nav>
</template>

<style scoped>
.umc-lang-switch {
  order: 10;
  display: inline-flex;
  align-items: center;
  align-self: center;
  gap: 2px;
  height: 30px;
  margin-left: 8px;
  padding: 3px;
  border: 1px solid var(--vp-c-divider);
  border-radius: 8px;
  background: var(--vp-c-bg-alt);
  box-sizing: border-box;
}
.umc-lang-switch a {
  display: inline-flex;
  align-items: center;
  height: 22px;
  padding: 0 8px;
  border-radius: 5px;
  color: var(--vp-c-text-3);
  font-size: 11.5px;
  font-weight: 650;
  line-height: 1;
  text-decoration: none;
  transition: background 0.15s ease, color 0.15s ease;
}
.umc-lang-switch a:hover {
  color: var(--vp-c-text-1);
}
.umc-lang-switch a.active {
  background: var(--vp-c-brand-soft);
  color: var(--vp-c-brand-1);
}

@media (max-width: 767px) {
  .umc-lang-switch {
    order: 0;
    margin-left: 4px;
  }
}

@media (max-width: 640px) {
  .umc-lang-switch {
    margin-left: 4px;
  }
  .umc-lang-switch a {
    padding: 0 6px;
  }
}
</style>
