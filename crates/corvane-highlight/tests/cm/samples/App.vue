<!-- App.vue: root component — ünïcödé comment
     spanning two lines -->
<template>
  <div id="app" :class="{ dark: isDark, 'is-mobile': mobile }" @click.stop="toggle">
    <header class="app-header">
      <h1>{{ title | capitalize }}</h1>
      <p v-if="user">Welcome back, {{ user.name }}! You have {{ unread }} messages.</p>
      <p v-else-if="loading">Loading… {{ progress }}%</p>
      <p v-else>Please <a href="#/login" @click.prevent="login()">sign in</a>.</p>
    </header>
    <ul class="todo-list">
      <li v-for="(item, index) in items" :key="item.id" :class="{ done: item.done }">
        <input type="checkbox" v-model="item.done" :id="'todo-' + index">
        <label :for="'todo-' + index">{{ index + 1 }}. {{ item.text }}</label>
        <button @click="remove(index)" :disabled="busy">&times;</button>
      </li>
    </ul>
    <template v-if="showFooter">
      <footer>{{ items.filter(i => !i.done).length }} left</footer>
    </template>
	<span>Tabbed {{ tabbed }}</span>
    <textarea v-model="draft">{{ not a mustache in rcdata? }}</textarea>
    <my-component v-bind:prop.sync="value" v-on:custom-event="handler($event)" />
    <p>Unterminated {{ mustache here
    and {{ another }} on the next line</p>
    <p>Empty {{}} and nested {{ {a: 1} }} braces</p>
    <script>var inline = "template script";</script>
    <style>.inline { color: red; }</style>
  </div>
</template>

<script>
import { ref, computed } from 'vue'
import TodoItem from './components/TodoItem.vue'

export default {
  name: 'App',
  components: { TodoItem },
  props: {
    title: { type: String, default: 'Todos' },
    mobile: Boolean,
  },
  data() {
    return {
      items: [],
      draft: '',
      isDark: false,
      busy: false,
      unread: 0x1F,
      progress: 12.5e-1,
    }
  },
  computed: {
    remaining() {
      return this.items.filter(item => !item.done).length
    },
  },
  methods: {
    async toggle(event) {
      this.isDark = !this.isDark
      /* multi-line
         comment */
      const re = /^\s*todo:(.*)$/gi
      console.log(`toggled ${this.isDark} at ${new Date()}`)
      await this.$nextTick()
    },
    remove(index) {
      this.items.splice(index, 1) // remove one
    },
  },
}
</script>

<style>
@import url("https://fonts.example.com/css?family=Inter");
#app {
  font-family: Inter, -apple-system, sans-serif;
  color: #2c3e50;
  margin: 0 auto !important;
}
.todo-list li.done label {
  text-decoration: line-through;
  opacity: .5;
}
@media (max-width: 600px) {
  .app-header h1 { font-size: 1.2em; }
}
/* unterminated comment
</style>

<style scoped lang="scss">
$primary: #42b983;
$radius: 4px;

@mixin rounded($r: $radius) {
  border-radius: $r;
}

.app-header {
  background: lighten($primary, 10%);
  @include rounded;
  &:hover { background: darken($primary, 5%); }
  h1 {
    // scss line comment
    font-weight: bold;
    #{$prop}-left: 1px;
  }
}
</style>

<style lang="less">
@base: #f938ab;
.bordered(@width: 2px) {
  border: @width solid black;
}
#header {
  color: darken(@base, 10%);
  .bordered(4px);
  .navigation { font-size: 12px; }
}
// less comment
</style>
