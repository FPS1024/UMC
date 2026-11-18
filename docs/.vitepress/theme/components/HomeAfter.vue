<script setup lang="ts">
import { computed } from 'vue'
import { useData, withBase } from 'vitepress'

const { page } = useData()
const isChinese = computed(() => page.value.relativePath.startsWith('zh/'))
</script>

<template>
  <div v-if="!isChinese" class="umc-home-extra">
    <section class="umc-section umc-surfaces">
      <div class="umc-section-head">
        <p class="umc-eyebrow">One engine, three surfaces</p>
        <h2>Pick the integration that fits your app</h2>
        <p class="umc-lead">
          Every surface delegates to the same <code>umc-core</code> implementation, so behavior and
          state stay consistent whether you embed UMC or run it beside your process.
        </p>
      </div>
      <div class="umc-surface-grid">
        <article class="umc-surface-card">
          <div class="umc-surface-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
              <path d="M7 8 3 12l4 4M17 8l4 4-4 4M14 5l-4 14" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </div>
          <h3>Rust SDK</h3>
          <p>Typed methods for versions, instances, tasks, Java, and processes — in your process.</p>
          <code>umc-sdk</code>
        </article>
        <article class="umc-surface-card">
          <div class="umc-surface-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
              <circle cx="12" cy="12" r="8.5" />
              <path d="M3.5 12h17M12 3.5c2.4 2.4 3.5 5.3 3.5 8.5S14.4 18.1 12 20.5C9.6 18.1 8.5 15.2 8.5 12S9.6 5.9 12 3.5Z" />
            </svg>
          </div>
          <h3>Local daemon</h3>
          <p>Newline-delimited JSON-RPC over a Unix socket or Windows named pipe. No TCP listener.</p>
          <code>umcd</code>
        </article>
        <article class="umc-surface-card">
          <div class="umc-surface-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
              <rect x="3" y="4" width="18" height="16" rx="3" />
              <path d="m7 9 3 3-3 3M12.5 15H17" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </div>
          <h3>CLI</h3>
          <p>Script installs, inspect tasks, and launch offline clients without writing code.</p>
          <code>umc</code>
        </article>
      </div>
    </section>

    <section class="umc-section umc-quickstart">
      <div class="umc-quickstart-copy">
        <p class="umc-eyebrow">Quickstart</p>
        <h2>From zero to a running client</h2>
        <p class="umc-lead">
          Create an isolated instance, verify every downloaded file, provision Java if needed, and
          launch the game — all from the terminal.
        </p>
        <div class="umc-inline-links">
          <a class="umc-inline-link" :href="withBase('/en/guide/quickstart')">
            Read the quickstart
            <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.8">
              <path d="M4 10h12M11 5l5 5-5 5" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </a>
          <a class="umc-inline-link" :href="withBase('/en/guide/user-guide')">User guide</a>
        </div>
      </div>
      <div class="umc-code-card">
        <div class="umc-code-card__bar">
          <span class="umc-code-dot umc-code-dot--red"></span>
          <span class="umc-code-dot umc-code-dot--yellow"></span>
          <span class="umc-code-dot umc-code-dot--green"></span>
          <span>bash</span>
          <span class="umc-code-card__tag">Linux x86_64</span>
        </div>
        <pre><code><span class="umc-c-prompt">$</span> umc version list
<span class="umc-c-prompt">$</span> umc instance create Survival 1.21.1
<span class="umc-c-out">→ instance-8f14d9 created</span>
<span class="umc-c-prompt">$</span> umc instance install instance-8f14d9
<span class="umc-c-prompt">$</span> umc java install 1.21.1
<span class="umc-c-prompt">$</span> umc launch instance-8f14d9 Steve --offline
<span class="umc-c-out">→ Minecraft 1.21.1 running</span></code></pre>
      </div>
    </section>

    <section class="umc-section umc-architecture">
      <div class="umc-section-head umc-section-head--center">
        <p class="umc-eyebrow">Architecture</p>
        <h2>Headless by design</h2>
        <p class="umc-lead">
          UMC owns the launcher work. Your application owns the interface, accounts, and product
          experience.
        </p>
      </div>
      <div class="umc-flow">
        <div class="umc-flow-node">
          <span class="umc-flow-kicker">01</span>
          <strong>Your application</strong>
          <p>Desktop GUI, server tool, or automation script.</p>
          <div class="umc-flow-tags"><span>UI</span><span>accounts</span></div>
        </div>
        <div class="umc-flow-arrow" aria-hidden="true">
          <svg viewBox="0 0 40 16" fill="none" stroke="currentColor" stroke-width="1.6">
            <path d="M2 8h34M30 3l6 5-6 5" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </div>
        <div class="umc-flow-node umc-flow-node--core">
          <span class="umc-flow-kicker">02</span>
          <strong>umc-core</strong>
          <p>Install, verify, launch, supervise, persist.</p>
          <div class="umc-flow-tags"><span>Rust</span><span>local-first</span><span>no telemetry</span></div>
        </div>
        <div class="umc-flow-arrow" aria-hidden="true">
          <svg viewBox="0 0 40 16" fill="none" stroke="currentColor" stroke-width="1.6">
            <path d="M2 8h34M30 3l6 5-6 5" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </div>
        <div class="umc-flow-node">
          <span class="umc-flow-kicker">03</span>
          <strong>Minecraft</strong>
          <p>Java Edition, installed into isolated instances.</p>
          <div class="umc-flow-tags"><span>vanilla</span><span>offline</span></div>
        </div>
      </div>
    </section>

    <section class="umc-section umc-status">
      <div class="umc-status-card umc-status-card--now">
        <div class="umc-status-head">
          <span class="umc-status-pill umc-status-pill--live"><span></span>Available now</span>
          <h2>What ships in v0.1.0</h2>
        </div>
        <ul class="umc-checklist">
          <li>Vanilla version discovery and verified downloads</li>
          <li>Isolated instances with config migration and repair</li>
          <li>Explicit offline launch with process supervision</li>
          <li>Managed Mojang Java runtime provisioning</li>
          <li>Persistent install tasks with progress and cancellation</li>
          <li>JSON-RPC, local daemon, and Rust SDK parity</li>
        </ul>
      </div>
      <div class="umc-status-card umc-status-card--next">
        <div class="umc-status-head">
          <span class="umc-status-pill umc-status-pill--next"><span></span>In development</span>
          <h2>On the roadmap</h2>
        </div>
        <ul class="umc-roadmap">
          <li><span class="umc-roadmap-dot"></span>Microsoft account authentication and online play</li>
          <li><span class="umc-roadmap-dot"></span>Fabric, Forge, and NeoForge install and launch</li>
          <li><span class="umc-roadmap-dot"></span>Mod, modpack, and resource pack management</li>
          <li><span class="umc-roadmap-dot"></span>Windows and macOS ports</li>
          <li><span class="umc-roadmap-dot"></span>Live log and crash-report streaming</li>
        </ul>
        <a class="umc-status-link" :href="withBase('/en/reference/release')">Read the release policy →</a>
      </div>
    </section>

    <section class="umc-cta">
      <div class="umc-cta-glow"></div>
      <div class="umc-cta-inner">
        <p class="umc-eyebrow">Start building</p>
        <h2>Ship your launcher on UMC.</h2>
        <p>
          Download the Linux build, follow the quickstart, or integrate the engine directly into
          your Rust application.
        </p>
        <div class="umc-cta-actions">
          <a
            class="umc-button umc-button--primary"
            href="https://github.com/FPS1024/UMC/releases/latest"
          >
            <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.8">
              <path d="M10 3v10M6 9l4 4 4-4M4 16h12" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
            Download for Linux
          </a>
          <a class="umc-button" :href="withBase('/en/guide/quickstart')">
            Read the docs
          </a>
          <a class="umc-button umc-button--ghost" href="https://github.com/FPS1024/UMC">
            <svg viewBox="0 0 24 24" fill="currentColor">
              <path d="M12 .5C5.73.5.5 5.73.5 12c0 5.08 3.29 9.39 7.86 10.91.58.11.79-.25.79-.56v-2.2c-3.2.7-3.87-1.36-3.87-1.36-.52-1.33-1.28-1.68-1.28-1.68-1.04-.71.08-.7.08-.7 1.15.08 1.76 1.18 1.76 1.18 1.03 1.76 2.7 1.25 3.36.96.1-.75.4-1.25.73-1.54-2.55-.29-5.24-1.28-5.24-5.69 0-1.26.45-2.29 1.18-3.1-.12-.29-.51-1.46.11-3.05 0 0 .96-.31 3.14 1.18a10.9 10.9 0 0 1 5.72 0c2.18-1.49 3.14-1.18 3.14-1.18.62 1.59.23 2.76.11 3.05.74.81 1.18 1.84 1.18 3.1 0 4.42-2.69 5.39-5.25 5.68.41.36.78 1.06.78 2.14v3.17c0 .31.21.68.8.56A11.51 11.51 0 0 0 23.5 12C23.5 5.73 18.27.5 12 .5Z" />
            </svg>
            GitHub
          </a>
        </div>
        <p class="umc-cta-note">MIT licensed · Linux x86_64 · No telemetry</p>
      </div>
    </section>
  </div>
</template>

<style scoped>
.umc-home-extra {
  width: 100%;
  max-width: 1152px;
  margin: 0 auto;
  padding: 0 24px 96px;
}

.umc-section {
  padding: 72px 0;
  border-top: 1px solid var(--vp-c-divider);
}

.umc-section-head {
  max-width: 660px;
  margin-bottom: 36px;
}
.umc-section-head--center {
  margin-left: auto;
  margin-right: auto;
  text-align: center;
}

.umc-eyebrow {
  margin: 0 0 10px;
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: var(--vp-c-brand-1);
}

.umc-section h2,
.umc-cta h2 {
  margin: 0 0 12px;
  font-size: clamp(26px, 3.4vw, 36px);
  line-height: 1.18;
  letter-spacing: -0.02em;
  font-weight: 700;
  color: var(--vp-c-text-1);
}

.umc-lead {
  margin: 0;
  font-size: 16px;
  line-height: 1.7;
  color: var(--vp-c-text-2);
}

.umc-surface-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 16px;
}

.umc-surface-card {
  position: relative;
  padding: 24px;
  border-radius: 16px;
  border: 1px solid var(--vp-c-divider);
  background: var(--vp-c-bg-soft);
  transition: border-color 0.25s, transform 0.25s, box-shadow 0.25s;
}
.umc-surface-card:hover {
  transform: translateY(-3px);
  border-color: var(--vp-c-brand-1);
  box-shadow: 0 20px 40px -28px rgba(52, 81, 178, 0.5);
}
.umc-surface-icon {
  display: grid;
  place-items: center;
  width: 42px;
  height: 42px;
  margin-bottom: 16px;
  border-radius: 12px;
  color: var(--vp-c-brand-1);
  background: var(--vp-c-brand-soft);
  border: 1px solid color-mix(in srgb, var(--vp-c-brand-1) 18%, transparent);
}
.umc-surface-icon svg { width: 22px; height: 22px; }
.umc-surface-card h3 {
  margin: 0 0 8px;
  font-size: 17px;
  font-weight: 650;
  color: var(--vp-c-text-1);
}
.umc-surface-card p {
  margin: 0 0 18px;
  font-size: 14px;
  line-height: 1.65;
  color: var(--vp-c-text-2);
}
.umc-surface-card code {
  font-family: var(--vp-font-family-mono);
  font-size: 12px;
  color: var(--vp-c-brand-1);
  background: var(--vp-c-brand-soft);
  border-radius: 6px;
  padding: 3px 8px;
}

.umc-quickstart {
  display: grid;
  grid-template-columns: minmax(0, 0.92fr) minmax(0, 1.08fr);
  gap: 40px;
  align-items: center;
}
.umc-inline-links {
  display: flex;
  flex-wrap: wrap;
  gap: 20px;
  margin-top: 24px;
}
.umc-inline-link {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 14px;
  font-weight: 600;
  color: var(--vp-c-brand-1);
  text-decoration: none;
}
.umc-inline-link svg { width: 16px; height: 16px; transition: transform 0.2s; }
.umc-inline-link:hover svg { transform: translateX(3px); }

.umc-code-card {
  border-radius: 16px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  background: linear-gradient(155deg, #141824, #0b0e14 70%);
  box-shadow: 0 32px 64px -36px rgba(3, 6, 18, 0.7);
  overflow: hidden;
}
.umc-code-card__bar {
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 11px 14px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  font-family: var(--vp-font-family-mono);
  font-size: 11px;
  color: #8b93a7;
}
.umc-code-dot { width: 9px; height: 9px; border-radius: 50%; }
.umc-code-dot--red { background: #ff5f57; }
.umc-code-dot--yellow { background: #febc2e; }
.umc-code-dot--green { background: #28c840; }
.umc-code-card__tag {
  margin-left: auto;
  padding: 2px 8px;
  border-radius: 999px;
  border: 1px solid rgba(124, 140, 255, 0.22);
  background: rgba(124, 140, 255, 0.1);
  color: #a8b1ff;
  font-size: 10px;
}
.umc-code-card pre {
  margin: 0;
  padding: 20px 22px 24px;
  overflow-x: auto;
}
.umc-code-card code {
  font-family: var(--vp-font-family-mono);
  font-size: 12.5px;
  line-height: 2;
  color: #dfe3ee;
}
.umc-c-prompt { color: #22c997; margin-right: 6px; }
.umc-c-out { color: #7c869b; }

.umc-flow {
  display: grid;
  grid-template-columns: 1fr 40px 1fr 40px 1fr;
  align-items: stretch;
  gap: 0;
}
.umc-flow-node {
  padding: 22px;
  border-radius: 16px;
  border: 1px solid var(--vp-c-divider);
  background: var(--vp-c-bg-soft);
}
.umc-flow-node--core {
  border-color: color-mix(in srgb, var(--vp-c-brand-1) 38%, var(--vp-c-divider));
  background: linear-gradient(160deg, var(--vp-c-brand-soft), var(--vp-c-bg-soft) 70%);
}
.umc-flow-kicker {
  display: block;
  margin-bottom: 34px;
  font-family: var(--vp-font-family-mono);
  font-size: 11px;
  color: var(--vp-c-text-3);
}
.umc-flow-node strong {
  display: block;
  margin-bottom: 6px;
  font-size: 16px;
  color: var(--vp-c-text-1);
}
.umc-flow-node p {
  margin: 0 0 14px;
  font-size: 13px;
  line-height: 1.6;
  color: var(--vp-c-text-2);
}
.umc-flow-tags { display: flex; flex-wrap: wrap; gap: 6px; }
.umc-flow-tags span {
  font-size: 10.5px;
  font-family: var(--vp-font-family-mono);
  color: var(--vp-c-text-2);
  background: var(--vp-c-default-soft);
  border-radius: 999px;
  padding: 2px 8px;
}
.umc-flow-arrow {
  display: grid;
  place-items: center;
  color: var(--vp-c-text-3);
}
.umc-flow-arrow svg { width: 30px; }

.umc-status {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 20px;
}
.umc-status-card {
  padding: 28px;
  border-radius: 18px;
  border: 1px solid var(--vp-c-divider);
  background: var(--vp-c-bg-soft);
}
.umc-status-card--next {
  background:
    linear-gradient(160deg, rgba(34, 201, 151, 0.06), transparent 55%),
    var(--vp-c-bg-soft);
}
.umc-status-head h2 {
  margin: 14px 0 20px;
  font-size: 22px;
  letter-spacing: -0.01em;
}
.umc-status-pill {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  border-radius: 999px;
  padding: 5px 10px;
}
.umc-status-pill span {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  display: inline-block;
}
.umc-status-pill--live {
  color: #18794e;
  background: rgba(16, 185, 129, 0.14);
}
.umc-status-pill--live span { background: #30a46c; }
.umc-status-pill--next {
  color: #915930;
  background: rgba(234, 179, 8, 0.13);
}
.umc-status-pill--next span { background: #da8b17; }
.dark .umc-status-pill--live { color: #3dd68c; }
.dark .umc-status-pill--next { color: #f9b44e; }

.umc-checklist,
.umc-roadmap {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.umc-checklist li {
  position: relative;
  padding-left: 27px;
  font-size: 14px;
  line-height: 1.55;
  color: var(--vp-c-text-2);
}
.umc-checklist li::before {
  content: "";
  position: absolute;
  left: 0;
  top: 3px;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: rgba(16, 185, 129, 0.16);
}
.umc-checklist li::after {
  content: "";
  position: absolute;
  left: 5px;
  top: 7px;
  width: 5px;
  height: 3px;
  border-left: 1.6px solid #30a46c;
  border-bottom: 1.6px solid #30a46c;
  transform: rotate(-45deg);
}
.umc-roadmap li {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 14px;
  line-height: 1.55;
  color: var(--vp-c-text-2);
}
.umc-roadmap-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #da8b17;
  flex: none;
}
.umc-status-link {
  display: inline-block;
  margin-top: 22px;
  font-size: 13.5px;
  font-weight: 600;
  color: var(--vp-c-brand-1);
  text-decoration: none;
}

.umc-cta {
  position: relative;
  margin-top: 24px;
  padding: 64px 32px;
  border-radius: 24px;
  border: 1px solid color-mix(in srgb, var(--vp-c-brand-1) 26%, var(--vp-c-divider));
  background:
    radial-gradient(circle at 20% 0%, rgba(94, 112, 255, 0.16), transparent 45%),
    radial-gradient(circle at 85% 100%, rgba(34, 201, 151, 0.14), transparent 45%),
    var(--vp-c-bg-soft);
  overflow: hidden;
  text-align: center;
}
.umc-cta-glow {
  position: absolute;
  inset: -40% 30% auto;
  height: 260px;
  background: radial-gradient(circle, rgba(94, 112, 255, 0.22), transparent 70%);
  filter: blur(40px);
  pointer-events: none;
}
.umc-cta-inner { position: relative; }
.umc-cta p {
  max-width: 560px;
  margin: 0 auto;
  font-size: 16px;
  line-height: 1.65;
  color: var(--vp-c-text-2);
}
.umc-cta-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 12px;
  margin: 28px 0 18px;
}
.umc-button {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 10px 18px;
  border-radius: 10px;
  border: 1px solid var(--vp-c-divider);
  background: var(--vp-c-bg);
  color: var(--vp-c-text-1);
  font-size: 14px;
  font-weight: 600;
  text-decoration: none;
  transition: border-color 0.2s, transform 0.2s, background 0.2s;
}
.umc-button:hover {
  transform: translateY(-1px);
  border-color: var(--vp-c-brand-1);
}
.umc-button svg { width: 17px; height: 17px; }
.umc-button--primary {
  border-color: transparent;
  background: #4f63e8;
  color: #fff;
  box-shadow: 0 10px 24px -10px rgba(79, 99, 232, 0.7);
}
.umc-button--primary:hover {
  background: #4356d6;
  border-color: transparent;
}
.umc-button--ghost {
  background: transparent;
}
.umc-cta-note {
  font-size: 12.5px !important;
  color: var(--vp-c-text-3) !important;
  font-family: var(--vp-font-family-mono);
}

@media (max-width: 960px) {
  .umc-surface-grid { grid-template-columns: 1fr; }
  .umc-quickstart { grid-template-columns: 1fr; gap: 28px; }
  .umc-flow { grid-template-columns: 1fr; gap: 12px; }
  .umc-flow-arrow { transform: rotate(90deg); padding: 2px 0; }
  .umc-status { grid-template-columns: 1fr; }
}

@media (max-width: 640px) {
  .umc-home-extra { padding: 0 18px 64px; }
  .umc-section { padding: 52px 0; }
  .umc-cta { padding: 48px 20px; }
  .umc-cta-actions { flex-direction: column; align-items: stretch; }
  .umc-button { justify-content: center; }
}
</style>
