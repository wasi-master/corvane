import React, { useState, useEffect } from 'react';
import PropTypes from 'prop-types';

// A JSX component
export default function TodoList({ items, onToggle, title = 'Todos' }) {
  const [filter, setFilter] = useState('all');
  const visible = items.filter(item => filter === 'all' || item.done === (filter === 'done'));

  useEffect(() => {
    document.title = `${visible.length} items`;
  }, [visible.length]);

  return (
    <div className="todo-list" data-count={visible.length}>
      <h1>{title} &amp; more</h1>
      {/* a JSX comment */}
      <input type="text" value={filter} onChange={e => setFilter(e.target.value)} disabled />
      <ul>
        {visible.map((item, i) => (
          <li key={item.id} className={item.done ? 'done' : ''} onClick={() => onToggle(item.id)}>
            {i + 1}. {item.text}
            {item.tags && <span className="tags">{item.tags.join(', ')}</span>}
          </li>
        ))}
      </ul>
      <>
        <Fragment.Child prop="x" {...rest} />
      </>
      <p style={{ color: 'red', fontSize: 12 }}>
        Café — naïve text with {"string"} and {`template ${nested}`}
      </p>
      {cond ? <Yes /> : <No reason="none" />}
      <Multi
        a="1"
        b={2} // trailing comment in tag
        /* block comment in tag */
        c
      />
    </div>
  );
}

TodoList.propTypes = {
  items: PropTypes.array.isRequired,
};

const el = <App />;
const ratio = a < b ? 1 : 2;
const html = <div dangerouslySetInnerHTML={{ __html: markup }}></div>;
function render() {
  return <Wrapper><Inner value={x > 1 && y < 2} /></Wrapper>;
}
class Widget extends React.Component {
  render() {
    const { label } = this.props;
    return <button onClick={this.handleClick}>{label}</button>;
  }
}
