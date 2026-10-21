import { ref } from "vue";

export type AppViewKey = "projects" | "settings";
export type ProjectViewKey = "graph" | "mr" | "gitlab" | "ai";
export type ViewKey = AppViewKey | ProjectViewKey;

export const activeView = ref<ViewKey>("projects");

export function navigate(view: ViewKey) {
  activeView.value = view;
}
