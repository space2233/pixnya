<script lang="ts">
  import { page } from "$app/state";

  let authorsActive = $derived(page.url.pathname.startsWith("/following/users"));
</script>

<nav class="following-tabs" aria-label="关注内容">
  <a href="/following" class:active={!authorsActive} aria-current={!authorsActive ? "page" : undefined}>最新作品</a>
  <a href="/following/users" class:active={authorsActive} aria-current={authorsActive ? "page" : undefined}>关注作者</a>
</nav>

<style>
  .following-tabs {
    display: flex;
    width: fit-content;
    gap: 4px;
    margin: 20px 0 4px;
    padding: 4px;
    border-radius: 22px;
    background: #eef1f3;
  }

  a {
    min-width: 92px;
    padding: 9px 17px;
    color: #697278;
    border-radius: 18px;
    font-size: 9px;
    font-weight: 700;
    text-align: center;
    text-decoration: none;
  }

  a.active {
    color: #fff;
    background: var(--pixiv-blue);
    box-shadow: 0 4px 12px rgba(0, 150, 250, .18);
  }

  @media (max-width: 620px) {
    .following-tabs {
      width: 100%;
      margin-top: 12px;
    }

    a {
      flex: 1;
      min-width: 0;
      padding-inline: 10px;
      font-size: 10px;
    }
  }
</style>
