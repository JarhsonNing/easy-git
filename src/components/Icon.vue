<script setup lang="ts">
withDefaults(
  defineProps<{
    name: string;
    size?: number;
    strokeWidth?: number;
  }>(),
  { size: 18, strokeWidth: 1.8 },
);

const paths: Record<string, string[]> = {
  plus: ["M12 5v14", "M5 12h14"],
  minus: ["M5 12h14"],
  stash: ["M3 5h18v4H3z", "M5 9v10a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V9", "M10 13h4"],
  search: ["M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14Z", "m20 20-4-4"],
  refresh: [
    "M20 11a8 8 0 1 0-2.34 5.66",
    "M20 4v7h-7",
  ],
  settings: [
    "M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2Z",
    "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z",
  ],
  "arrow-left": ["M19 12H5", "m12 19-7-7 7-7"],
  "arrow-right": ["M5 12h14", "m12 5 7 7-7 7"],
  "arrow-up-right": ["M7 17 17 7", "M8 7h9v9"],
  close: ["M6 6l12 12", "M18 6 6 18"],
  "chevron-down": ["m6 9 6 6 6-6"],
  "chevron-right": ["m9 18 6-6-6-6"],
  list: ["M8 6h13", "M8 12h13", "M8 18h13", "M3 6h.01", "M3 12h.01", "M3 18h.01"],
  tree: ["M6 3v12", "M6 9h6a3 3 0 0 1 3 3v9", "M6 15h9", "M6 21h.01", "M18 15h.01"],
  "git-compare": [
    "M6 3v12",
    "M18 9a9 9 0 0 1-9 9",
    "M6 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z",
    "M18 6a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z",
    "m18 12 3-3-3-3",
  ],
  branch: [
    "M6 3v12",
    "M18 9a9 9 0 0 1-9 9",
    "M6 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z",
    "M18 6a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z",
  ],
  "git-merge": ["M6 3v12", "M6 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z", "M18 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z", "M9 18h3a6 6 0 0 0 6-6V9"],
  sparkles: [
    "m12 3 1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8L12 3Z",
    "M19 15.5 19.8 18l2.2.8-2.2.8-.8 2.4-.8-2.4-2.2-.8 2.2-.8.8-2.4Z",
  ],
  check: ["m5 12 4 4L19 6"],
  stop: ["M7 7h10v10H7z"],
  copy: [
    "M9 9h10v10H9z",
    "M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1",
  ],
  external: ["M14 4h6v6", "M20 4 10 14", "M18 13v5a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h5"],
  lock: ["M5 11h14v10H5z", "M8 11V8a4 4 0 0 1 8 0v3"],
  more: ["M5 12h.01", "M12 12h.01", "M19 12h.01"],
  folder: ["M3 6a2 2 0 0 1 2-2h5l2 3h7a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6Z"],
  clock: ["M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Z", "M12 7v5l3 2"],
  message: ["M21 15a4 4 0 0 1-4 4H8l-5 3V6a4 4 0 0 1 4-4h10a4 4 0 0 1 4 4v9Z"],
  file: ["M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z", "M14 2v6h6"],
  alert: ["M12 3 2.5 20h19L12 3Z", "M12 9v5", "M12 17h.01"],
  dot: ["M12 12h.01"],
  upload: ["M12 19V5", "m5 12 7-7 7 7"],
  download: ["M12 5v14", "m19 12-7 7-7-7"],
  tag: ["M3 11V4a1 1 0 0 1 1-1h7l10 10-8 8L3 11Z", "M7.5 7.5h.01"],
  pencil: ["M12 20h9", "M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z"],
  trash: ["M3 6h18", "M8 6V4h8v2", "M19 6l-1 14H6L5 6", "M10 11v6", "M14 11v6"],
  undo: ["M3 12a9 9 0 1 0 2.64-6.36L3 8", "M3 3v5h5"],
  commit: ["M12 17a5 5 0 1 0 0-10 5 5 0 0 0 0 10Z", "M2 12h5", "M17 12h5"],
  play: ["m7 4 13 8-13 8V4Z"],
  cloud: ["M17.5 19a4.5 4.5 0 0 0 .5-8.97A6 6 0 0 0 6.1 8.5 4 4 0 0 0 6.5 19h11Z"],
};
</script>

<template>
  <svg
    class="icon"
    :width="size"
    :height="size"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    :stroke-width="strokeWidth"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path v-for="(path, index) in paths[name] ?? paths.dot" :key="index" :d="path" />
  </svg>
</template>
