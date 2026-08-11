<script lang="ts">
  import { m } from "$lib/i18n";
</script>

<span class="skeleton-art" role="status" aria-label={m.thumbnail_loading()}>
  <i></i><i></i><i></i>
</span>

<style>
  .skeleton-art {
    position: absolute;
    inset: 0;
    overflow: hidden;
    background: linear-gradient(135deg, #f1f3f5 52%, #eceff1 52%);
  }

  .skeleton-art::after {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      105deg,
      transparent 35%,
      rgba(255, 255, 255, 0.72) 48%,
      transparent 61%
    );
    animation: sweep 1.5s infinite;
    content: "";
  }

  .skeleton-art i {
    position: absolute;
    border-radius: 999px;
    background: rgba(214, 219, 223, 0.72);
  }

  .skeleton-art i:nth-child(1) {
    bottom: 17%;
    left: 12%;
    width: 52%;
    height: 7%;
  }

  .skeleton-art i:nth-child(2) {
    bottom: 9%;
    left: 12%;
    width: 34%;
    height: 5%;
  }

  .skeleton-art i:nth-child(3) {
    top: 14%;
    right: 14%;
    width: 18%;
    aspect-ratio: 1;
  }

  @keyframes sweep {
    from { transform: translateX(-80%); }
    to { transform: translateX(80%); }
  }

  @media (prefers-reduced-motion: reduce) {
    .skeleton-art::after { animation: none; }
  }
</style>
