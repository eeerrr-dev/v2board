import 'react';

declare module 'react' {
  interface SelectHTMLAttributes<T> {
    placeholder?: string;
  }
}
