import * as React from 'react';
import type { FC, ReactNode } from 'react';

interface ButtonProps {
  label: string;
  onClick?: (event: React.MouseEvent<HTMLButtonElement>) => void;
  children?: ReactNode;
  variant: 'primary' | 'secondary';
}

type State = { count: number; items: Array<string> };

export const Button: FC<ButtonProps> = ({ label, onClick, children, variant = 'primary' }) => {
  const [count, setCount] = React.useState<number>(0);
  const ref = React.useRef<HTMLDivElement>(null);
  const handle = (e: React.MouseEvent<HTMLButtonElement>): void => {
    setCount(c => c + 1);
    onClick?.(e);
  };
  return (
    <div ref={ref} className={`btn btn-${variant}`}>
      <button type="button" onClick={handle} aria-label={label}>
        {label} ({count})
      </button>
      {children}
      {count > 0 && <Badge<number> value={count} />}
    </div>
  );
};

function identity<T,>(value: T): T {
  return value;
}
const generic = <T,>(x: T) => x;
const casted = value as unknown as HTMLElement;

export class Counter extends React.Component<ButtonProps, State> {
  state: State = { count: 0, items: [] };
  private timer?: number;
  render(): JSX.Element {
    const { label } = this.props;
    return <span title={label}>{this.state.count}</span>;
  }
}

enum Size { Small = 's', Large = 'l' }
export default Button;
