import type { ComponentProps } from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "../../lib/utils";
import { Tooltip } from "./tooltip";

const buttonVariants = cva("ui-button", {
  variants: {
    variant: {
      default: "ui-button-primary",
      secondary: "ui-button-secondary",
      destructive: "ui-button-destructive",
      ghost: "ui-button-ghost",
      link: "ui-button-link",
    },
    size: {
      default: "ui-button-default",
      sm: "ui-button-sm",
      icon: "ui-button-icon",
      "icon-sm": "ui-button-icon-sm",
    },
  },
  defaultVariants: { variant: "secondary", size: "default" },
});

function Button({
  className,
  variant,
  size,
  asChild = false,
  type = "button",
  title,
  ...props
}: ComponentProps<"button"> &
  VariantProps<typeof buttonVariants> & { asChild?: boolean }) {
  const Comp = asChild ? Slot : "button";
  const button = (
    <Comp
      data-slot="button"
      type={asChild ? undefined : type}
      className={cn(buttonVariants({ variant, size }), className)}
      {...props}
    />
  );
  return title ? <Tooltip content={title}>{button}</Tooltip> : button;
}

export { Button, buttonVariants };
