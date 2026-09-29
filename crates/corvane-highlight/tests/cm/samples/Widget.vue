<template lang="pug">
div.widget(:class="{ active: isActive }" @click="select")
  h2.widget__title {{ title }}
  // pug comment
  //- silent comment
  ul
    li(v-for="item in items" :key="item.id") #{item.label} — ünïcödé
  if showFooter
    footer.widget__footer
      | Plain text with {{ mustache }}
      span= total
  - var answer = 42
  p.
    A block of text
    over two lines
  button(type="button", disabled) Click
  :coffeescript
    square = (x) -> x * x
  :stylus
    .a
      color red
  <div class="raw">inline html</div>
</template>

<script lang="ts">
import { defineComponent, PropType } from 'vue'

interface Item {
  id: number
  label: string
  tags?: string[]
}

enum Mode { Idle = 0, Busy }

export default defineComponent({
  name: 'Widget',
  props: {
    items: { type: Array as PropType<Item[]>, required: true },
    title: String,
  },
  setup(props, { emit }) {
    const isActive = ref<boolean>(false)
    const total = computed((): number => props.items.length)
    function select(e: MouseEvent): void {
      isActive.value = !isActive.value
      emit('select', e)
    }
    return { isActive, total, select }
  },
})
</script>

<script lang="coffee">
# CoffeeScript block
class Greeter extends Base
  constructor: (@name = "world") ->
    super()
  greet: ->
    console.log "Hello, #{@name}!"
    ###
    block comment
    ###
    for i in [1..10] when i % 2 is 0
      yield i
module.exports = Greeter
</script>

<script type="text/coffeescript">
answer = if yes then 42 else off
</script>

<style lang="stylus">
// stylus comment
$accent = #ff4081
.widget
  padding 8px 16px
  border 1px solid $accent
  &:hover
    background lighten($accent, 20%)
  &__title
    font-size 1.5em
    +below(600px)
      font-size 1em
</style>

<style lang="postcss">
.unknown { color: blue; }
</style>

<style type="text/x-scss">
.nested { .inner { margin: 0 } }
</style>

<style type="text/plain">
this is not css { at: all }
</style>

<template lang="handlebars">
<div class="entry">
  {{#if author}}
    <h1>{{firstName}} {{lastName}}</h1>
  {{else}}
    {{! handlebars comment }}
    {{!-- dashed comment --}}
    <p>{{{rawHtml}}} and "{{escaped "quoted"}}"</p>
  {{/if}}
</div>
</template>

<template lang="mustache">
  <p>{{ unknown lang falls back to vue-template }}</p>
</template>

<custom-block>
  Some <b>custom</b> content {{ not highlighted }}
</custom-block>
<i18n>
{ "en": { "hello": "Hello" } }
</i18n>
