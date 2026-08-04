<script lang="ts">
  export type IconName =
    | "menu"
    | "home"
    | "image"
    | "book"
    | "compass"
    | "ranking"
    | "heart"
    | "search"
    | "bell"
    | "user"
    | "settings"
    | "download"
    | "history"
    | "shield";

  const paths: Record<IconName, string[]> = {
    menu: ["M4 7h16", "M4 12h16", "M4 17h16"],
    home: ["m3 11 9-7 9 7", "M5 10v10h14V10", "M9 20v-6h6v6"],
    image: [
      "M4 5h16v14H4z",
      "m4 16 4-4 3 3 3-4 6 6",
      "M9 9h.01",
    ],
    book: ["M4 5.5A3.5 3.5 0 0 1 7.5 2H12v18H7.5A3.5 3.5 0 0 0 4 23z", "M20 5.5A3.5 3.5 0 0 0 16.5 2H12v18h4.5A3.5 3.5 0 0 1 20 23z"],
    compass: ["M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Z", "m15 9-2 4-4 2 2-4z"],
    ranking: ["M5 20v-6h4v6", "M10 20V9h4v11", "M15 20V4h4v16"],
    heart: ["M20.8 4.6a5.5 5.5 0 0 0-7.8 0L12 5.7l-1-1.1a5.5 5.5 0 0 0-7.8 7.8l1 1L12 21l7.8-7.6a5.5 5.5 0 0 0 1-7.8Z"],
    search: ["M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16Z", "m21 21-4.35-4.35"],
    bell: ["M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9", "M10 21h4"],
    user: ["M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z", "M4 21a8 8 0 0 1 16 0"],
    settings: [
      "M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.09a2 2 0 0 1 1 1.74v.5a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.38a2 2 0 0 0-.73-2.73l-.15-.09a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2Z",
      "M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z",
    ],
    download: ["M12 3v12", "m7 10 5 5 5-5", "M5 21h14"],
    history: ["M3 12a9 9 0 1 0 3-6.7", "M3 4v5h5", "M12 7v5l3 2"],
    shield: ["M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10Z", "m9 12 2 2 4-5"],
  };

  let { name, size = 22 }: { name: IconName; size?: number } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.9"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#each paths[name] as path}
    <path d={path}></path>
  {/each}
</svg>
