<script>
  // Todo list with filtering and a derived count.
  import { onMount } from 'svelte';
  import TodoItem from './TodoItem.svelte';
  export let title = 'Todos';
  let todos = [];
  let filter = 'all', draft = '';

  $: visible = todos.filter((t) => filter === 'all' || (filter === 'done') === t.done);
  $: remaining = todos.filter((t) => !t.done).length;
  function add() {
    if (!draft.trim()) return;
    todos = [...todos, { id: Date.now(), text: draft.trim(), done: false }];
    draft = '';
  }
  onMount(async () => {
    const res = await fetch('/api/todos');
    todos = res.ok ? await res.json() : [];
  });
</script>

<h1>{title} <small>({remaining} left)</small></h1>
<form on:submit|preventDefault={add}>
  <input bind:value={draft} placeholder="What needs doing?" />
  <button type="submit" disabled={!draft}>Add</button>
</form>

{#if visible.length === 0}
  <p class="empty">Nothing to show.</p>
{:else}
  <ul>
    {#each visible as todo, i (todo.id)}
      <TodoItem {todo} index={i} on:toggle={() => (todo.done = !todo.done)} />
    {/each}
  </ul>
{/if}
<!-- Filter buttons -->
{#each ['all', 'open', 'done'] as f}
  <button class:active={filter === f} on:click={() => (filter = f)}>{f}</button>
{/each}
<style>
  h1 { font-size: 1.5rem; color: #333; }
  .empty { opacity: 0.6; font-style: italic; }
  button.active { border-bottom: 2px solid var(--accent, tomato); }
</style>
