import { useMemo, type ComponentProps } from "react";
import * as SliderPrimitive from "@radix-ui/react-slider";
import { cn } from "../../lib/utils";

function Slider({
  className,
  defaultValue,
  value,
  min = 0,
  max = 100,
  thumbProps,
  ...props
}: ComponentProps<typeof SliderPrimitive.Root> & {
  thumbProps?: ComponentProps<typeof SliderPrimitive.Thumb>;
}) {
  const values = useMemo(
    () => value ?? defaultValue ?? [min],
    [value, defaultValue, min],
  );
  return (
    <SliderPrimitive.Root
      data-slot="slider"
      className={cn("ui-slider", className)}
      defaultValue={defaultValue}
      value={value}
      min={min}
      max={max}
      {...props}
    >
      <SliderPrimitive.Track
        data-slot="slider-track"
        className="ui-slider-track"
      >
        <SliderPrimitive.Range
          data-slot="slider-range"
          className="ui-slider-range"
        />
      </SliderPrimitive.Track>
      {values.map((_, index) => (
        <SliderPrimitive.Thumb
          key={index}
          data-slot="slider-thumb"
          className="ui-slider-thumb"
          {...thumbProps}
        />
      ))}
    </SliderPrimitive.Root>
  );
}

export { Slider };
