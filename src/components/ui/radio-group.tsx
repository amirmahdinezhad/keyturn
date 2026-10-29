import type { ComponentProps } from "react";
import * as RadioGroupPrimitive from "@radix-ui/react-radio-group";
import { cn } from "../../lib/utils";

export const RadioGroup = RadioGroupPrimitive.Root;

export function RadioGroupItem({ className, ...props }: ComponentProps<typeof RadioGroupPrimitive.Item>) {
  return (
    <RadioGroupPrimitive.Item {...props} className={cn("ui-radio", className)} data-slot="radio">
      <RadioGroupPrimitive.Indicator className="ui-radio-indicator" />
    </RadioGroupPrimitive.Item>
  );
}
