<script setup lang="ts">
import { computed } from "vue";

const props = withDefaults(
  defineProps<{
    seed: string;
    animated?: boolean;
    label?: string;
  }>(),
  { animated: false, label: "" },
);

const PALETTE = ["#1F6E6A", "#C9563C", "#B47A2C", "#765C88", "#4D7458", "#5C7890"];

const templates = [
  {
    contours: [
      "M8 96C42 82 70 100 110 89C146 79 168 91 198 80",
      "M18 32C48 23 78 36 108 28C141 19 165 31 194 22",
    ],
    branches: [
      "M-8 66C30 66 40 62 66 60C96 57 112 54 138 52C164 50 180 45 208 37",
      "M60 61C78 50 92 32 120 28C146 24 168 21 208 13",
      "M86 59C104 66 120 82 150 88C172 92 188 92 208 89",
      "M118 53C132 42 150 42 170 46C184 49 196 54 208 58",
    ],
    joints: [
      [60, 61],
      [86, 59],
      [118, 53],
    ],
    end: [190, 38],
  },
  {
    contours: [
      "M12 98C46 86 74 102 110 93C144 84 170 95 197 86",
      "M6 28C38 21 74 31 108 23C140 15 170 25 199 17",
    ],
    branches: [
      "M-8 60C30 60 42 58 64 58C96 58 128 61 158 58C178 56 192 53 208 51",
      "M64 58C82 44 100 25 132 20C160 16 184 17 208 11",
      "M96 58C112 72 132 93 164 98C182 101 196 97 208 92",
      "M138 57C152 46 168 38 190 34C198 33 204 32 208 32",
    ],
    joints: [
      [64, 58],
      [96, 58],
      [138, 57],
    ],
    end: [194, 51],
  },
  {
    contours: [
      "M8 100C40 88 72 105 106 95C142 84 170 97 199 87",
      "M20 25C52 18 80 30 112 21C144 13 170 23 198 15",
    ],
    branches: [
      "M-8 70C28 70 36 48 64 46C92 44 104 68 132 66C158 64 176 43 208 38",
      "M64 46C78 36 88 20 116 16C146 12 174 16 208 9",
      "M104 62C116 79 134 96 166 98C186 99 198 94 208 88",
      "M36 63C50 54 58 44 78 40",
    ],
    joints: [
      [64, 46],
      [104, 62],
    ],
    end: [194, 38],
  },
  {
    contours: [
      "M10 97C44 86 76 101 112 91C148 81 172 94 199 85",
      "M8 30C42 23 72 34 106 26C140 18 168 28 198 20",
    ],
    branches: [
      "M-8 58C24 58 44 54 64 54C84 54 96 61 116 58C142 54 162 47 208 43",
      "M64 54C76 40 92 30 112 26C142 20 172 17 208 13",
      "M86 56C98 72 112 86 136 90C160 94 184 90 208 85",
      "M116 56C132 44 152 38 176 36C188 35 198 36 208 38",
    ],
    joints: [
      [64, 54],
      [86, 56],
      [116, 56],
    ],
    end: [194, 43],
  },
];

function hash(value: string): number {
  let result = 2166136261;
  for (let index = 0; index < value.length; index += 1) {
    result ^= value.charCodeAt(index);
    result = Math.imul(result, 16777619);
  }
  return result >>> 0;
}

const seedValue = computed(() => hash(props.seed || "easy-git"));
const template = computed(() => templates[seedValue.value % templates.length]);
const palette = computed(() => {
  const offset = seedValue.value % PALETTE.length;
  return PALETTE.map((_, index) => PALETTE[(index + offset) % PALETTE.length]);
});
const uid = computed(() => `delta-${seedValue.value.toString(36)}`);
</script>

<template>
  <figure
    class="delta"
    :class="{ 'delta-animated': animated }"
    :aria-label="label || undefined"
    :role="label ? 'img' : undefined"
  >
    <svg viewBox="0 0 200 112" preserveAspectRatio="xMidYMid meet" aria-hidden="true">
      <defs>
        <clipPath :id="uid">
          <rect x="0" y="0" width="200" height="112" rx="0" />
        </clipPath>
      </defs>
      <g :clip-path="`url(#${uid})`" class="delta-contours">
        <path
          v-for="(contour, index) in template.contours"
          :key="`contour-${index}`"
          :d="contour"
          pathLength="1"
        />
      </g>
      <g class="delta-channels">
        <path
          v-for="(channel, index) in template.branches"
          :key="`channel-${index}`"
          :d="channel"
          :stroke="palette[index % palette.length]"
          pathLength="1"
          :style="{ '--delay': `${index * 70}ms` }"
        />
      </g>
      <g class="delta-joints">
        <circle
          v-for="(joint, index) in template.joints"
          :key="`joint-${index}`"
          :cx="joint[0]"
          :cy="joint[1]"
          r="3.4"
          :stroke="palette[(index + 1) % palette.length]"
        />
      </g>
      <circle
        class="delta-end"
        :cx="template.end[0]"
        :cy="template.end[1]"
        r="4.6"
        :fill="palette[0]"
      />
      <path
        class="delta-flow"
        :d="template.branches[0]"
        pathLength="1"
        :stroke="palette[0]"
      />
    </svg>
  </figure>
</template>
