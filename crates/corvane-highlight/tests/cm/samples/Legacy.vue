<template lang="vue-template">
  <section>
    <h3>{{ heading }}</h3>
    <template lang="pug">
      <p>nested template tags are just html here {{ x }}</p>
    </template>
    <p>after nested {{ y }}</p>
  </section>
</template>

<template type="text/x-pug">
p Pug via type attribute
  span.x= value
</template>

<template type="text/x-handlebars-template">
<ul>{{#each items}}<li>{{this}}</li>{{/each}}</ul>
</template>

<script lang="babel">
const double = x => x * 2;
export default { data: () => ({ double }) };
</script>

<script type="text/babel">
let [a, , b] = [1, 2, 3];
</script>

<script type="text/ecmascript-6">
class A { static #priv = 1n; }
</script>

<script type="application/x-coffeescript">
list = (num for num in [1..5])
</script>

<script type="text/x-template" id="tmpl">
  <div>{{ this is a plain text/plain block }}</div>
</script>

<script lang="coffeescript">
unterminated = "string with #{interp
more = 'after'
</script>

<script>
	// tab-indented
	var unterminated = "oops
	var s = 'ünï';
</script>

<style type="text/x-less">
@c: red;
.x { color: @c; }
</style>

<style type="text/stylus">
.y
  color blue
</style>

<style lang="SCSS">
.upper { a: b; }
</style>

<style lang="sass">
.sass-block
  color: red
  margin: 0 auto
</style>
