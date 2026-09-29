// JSX edge cases
const a = <div
  // line comment inside a tag
  /* block comment
     spanning lines */ id="x"
  data-flag
  onClick={() => {
    if (x < y) { go(); }
  }}
  style={{ margin: 0 }}>
  text with {"{braces}"} and &nbsp; entity
  {list.map(item => <Item key={item} {...item} />)}
  {/* comment child */}
  {
    // comment in expression
    value
  }
</div>;

const fragment = <>
  <First />
  <Second attr={<Nested inside="attr" />} />
</>;

const tagInTag = <Outer attr=<Inner /> other="1" />;
const namespaced = <svg:rect xlink:href="#a" />;
const member = <Foo.Bar.Baz prop={1}></Foo.Bar.Baz>;
const mismatched = <a></b>;
const conditional = flag && <Shown /> || <Hidden />;
const inTernary = ok ? (
  <Yes>
    <Deep level={1}>{`template ${inside} jsx`}</Deep>
  </Yes>
) : null;
const lessThan = count < limit;
const genericArrow = <T,>(value: T): T => value;
const genericCall = useState<string>('');
function Comp<P extends object>(props: P & { children?: React.ReactNode }) {
  return <section {...props}>{props.children}</section>;
}
const arrowJsx = () => <span>arrow</span>;
const arrayJsx = [<li key="1">one</li>, <li key="2">two</li>];
const returnsJsx = function () {
  return (
    <ul>
      <li>ünïcödé — 日本語</li>
    </ul>
  );
};
const unclosedExpression = <p>{open
  still in js }</p>;
const afterAll = 1 < 2;
export default <App />;
