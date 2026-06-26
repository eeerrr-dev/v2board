import { Input, type InputProps } from '@/components/ui/input';

export function PasswordField({ className, ...props }: InputProps) {
  return (
    <Input
      {...props}
      type="password"
      className={className}
    />
  );
}
