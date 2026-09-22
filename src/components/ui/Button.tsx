import type { AnchorHTMLAttributes, ButtonHTMLAttributes, ReactNode } from "react";
import { Link, type To } from "react-router-dom";

type ButtonVariant = "primary" | "secondary" | "ghost";

type BaseButtonProps = {
  children: ReactNode;
  variant?: ButtonVariant;
  iconOnly?: boolean;
  className?: string;
};

type NativeButtonProps = BaseButtonProps & ButtonHTMLAttributes<HTMLButtonElement> & { to?: never };
type LinkButtonProps = BaseButtonProps & AnchorHTMLAttributes<HTMLAnchorElement> & { to: To };

function getClassName(variant: ButtonVariant, iconOnly: boolean, className?: string) {
  return ["button", `button--${variant}`, iconOnly ? "button--icon" : "", className]
    .filter(Boolean)
    .join(" ");
}

export type ButtonProps = NativeButtonProps | LinkButtonProps;

export default function Button(props: ButtonProps) {
  if ("to" in props && props.to !== undefined) {
    const { to, children, variant = "secondary", iconOnly = false, className, ...linkProps } = props;
    return (
      <Link {...linkProps} to={to} className={getClassName(variant, iconOnly, className)}>
        {children}
      </Link>
    );
  }

  const { children, variant = "secondary", iconOnly = false, className, type = "button", ...buttonProps } = props;
  return (
    <button {...buttonProps} type={type} className={getClassName(variant, iconOnly, className)}>
      {children}
    </button>
  );
}
