import { defineConfig } from 'vitepress'

const base = '/UMC/'

const landingRedirectScript = `(function () {
  var base = ${JSON.stringify(base)};
  var path = location.pathname;
  var landing = path === base || path === base.replace(/\\/$/, '') ||
    path === base + 'index.html' || path === base + 'zh/' ||
    path === base + 'zh' || path === base + 'zh/index.html';
  if (landing) document.documentElement.classList.add('umc-landing');
})();`

export default defineConfig({
  lang: 'en-US',
  title: 'UMC',
  titleTemplate: ':title | Universal Minecraft Core',
  description:
    'UMC is a headless Minecraft: Java Edition launcher engine written in Rust. Install, launch, and supervise the game from your own application.',
  base,
  cleanUrls: true,
  lastUpdated: true,
  head: [
    ['script', {}, landingRedirectScript],
    ['link', { rel: 'icon', type: 'image/png', href: '/UMC/appicon.png' }],
    ['link', { rel: 'apple-touch-icon', href: '/UMC/appicon-180.png' }],
    ['link', { rel: 'preconnect', href: 'https://fonts.googleapis.com' }],
    ['link', { rel: 'preconnect', href: 'https://fonts.gstatic.com', crossorigin: '' }],
    [
      'link',
      {
        rel: 'stylesheet',
        href: 'https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800&family=JetBrains+Mono:wght@400;500;600&display=swap',
      },
    ],
    ['meta', { name: 'theme-color', content: '#ffffff', media: '(prefers-color-scheme: light)' }],
    ['meta', { name: 'theme-color', content: '#0b0d12', media: '(prefers-color-scheme: dark)' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:title', content: 'UMC — Universal Minecraft Core' }],
    [
      'meta',
      {
        property: 'og:description',
        content:
          'One Core. Every Launcher. A headless Minecraft: Java Edition launcher engine written in Rust.',
      },
    ],
    ['meta', { property: 'og:site_name', content: 'Universal Minecraft Core' }],
    ['meta', { property: 'og:image', content: 'https://fps1024.github.io/UMC/appicon.png' }],
    ['meta', { name: 'twitter:card', content: 'summary_large_image' }],
    ['meta', { name: 'twitter:image', content: 'https://fps1024.github.io/UMC/appicon.png' }],
  ],
  markdown: {
    theme: { light: 'github-dark', dark: 'github-dark' },
  },
  themeConfig: {
    logo: '/appicon.png',
    siteTitle: 'Universal Minecraft Core',
    nav: [
      { text: 'Guide', link: '/guide/quickstart', activeMatch: '^/guide/' },
      { text: 'Development', link: '/development/', activeMatch: '^/development/' },
      { text: 'Reference', link: '/reference/rpc-v1', activeMatch: '^/reference/' },
      {
        text: 'Download',
        link: 'https://github.com/FPS1024/UMC/releases/latest',
      },
    ],
    sidebar: {
      '/guide/': [
        {
          text: 'Getting started',
          items: [
            { text: 'Quickstart', link: '/guide/quickstart' },
            { text: 'Installation', link: '/guide/install' },
            { text: 'User guide', link: '/guide/user-guide' },
          ],
        },
        {
          text: 'Reference',
          items: [
            { text: 'JSON-RPC v1', link: '/reference/rpc-v1' },
            { text: 'Release policy', link: '/reference/release' },
          ],
        },
      ],
      '/development/': [
        {
          text: 'Development',
          items: [
            { text: 'Integration guide', link: '/development/' },
            { text: 'Rust SDK', link: '/development/sdk' },
            { text: 'Architecture', link: '/development/architecture' },
          ],
        },
        {
          text: 'Reference',
          items: [
            { text: 'JSON-RPC v1', link: '/reference/rpc-v1' },
            { text: 'Release notes — v0.1.0', link: '/reference/releases/v0.1.0' },
          ],
        },
      ],
      '/reference/': [
        {
          text: 'Reference',
          items: [
            { text: 'JSON-RPC v1', link: '/reference/rpc-v1' },
            { text: 'Release policy', link: '/reference/release' },
            { text: 'Legal and privacy', link: '/reference/legal-and-privacy' },
            { text: 'Release notes — v0.1.0', link: '/reference/releases/v0.1.0' },
          ],
        },
        {
          text: 'Develop',
          items: [
            { text: 'Integration guide', link: '/development/' },
            { text: 'Rust SDK', link: '/development/sdk' },
            { text: 'Architecture', link: '/development/architecture' },
          ],
        },
      ],
    },
    socialLinks: [
      { icon: 'github', link: 'https://github.com/FPS1024/UMC' },
      {
        icon: {
          svg: '<svg role="img" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="currentColor" d="M20.317 4.37a19.79 19.79 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028c.462-.63.874-1.295 1.226-1.994a.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128c.126-.094.252-.192.372-.291a.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.099.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.892.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.334-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.334-.946 2.418-2.157 2.418z"/></svg>',
        },
        link: 'https://discord.gg/d5qKvMp9Wc',
        ariaLabel: 'Discord',
      },
    ],
    search: {
      provider: 'local',
    },
    editLink: {
      pattern: 'https://github.com/FPS1024/UMC/edit/main/docs/:path',
      text: 'Edit this page on GitHub',
    },
    outline: {
      level: [2, 3],
      label: 'On this page',
    },
    docFooter: {
      prev: 'Previous',
      next: 'Next',
    },
    darkModeSwitchLabel: 'Appearance',
    lightModeSwitchTitle: 'Switch to light theme',
    darkModeSwitchTitle: 'Switch to dark theme',
    sidebarMenuLabel: 'Menu',
    returnToTopLabel: 'Back to top',
    lastUpdated: {
      text: 'Updated',
      formatOptions: { dateStyle: 'medium', timeStyle: 'short' },
    },
    footer: {
      message:
        'UMC is an independent project. It is not an official Minecraft product and is not approved by or associated with Mojang or Microsoft.',
      copyright: 'Released under the MIT License · © 2026 UMC contributors',
    },
  },
})
