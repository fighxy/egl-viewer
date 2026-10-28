import { cn } from "cn";
import { Slider as SliderPrimitive } from "radix-ui";
import type * as React from "react";

/** A single-thumb slider; `aria-label` names the thumb for assistive technology. */
function Slider({
  className,
  "aria-label": label,
  ...props
}: React.ComponentProps<typeof SliderPrimitive.Root>) {
  return (
    <SliderPrimitive.Root
      data-slot="slider"
      className={cn(
        "relative flex w-full touch-none items-center select-none data-[disabled]:opacity-50",
        className,
      )}
      {...props}
    >
      <SliderPrimitive.Track className="relative h-1 w-full grow overflow-hidden rounded-full bg-muted">
        <SliderPrimitive.Range className="absolute h-full bg-primary" />
      </SliderPrimitive.Track>
      <SliderPrimitive.Thumb
        aria-label={label}
        className="block size-3.5 rounded-full border border-primary bg-background shadow-sm outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
      />
    </SliderPrimitive.Root>
  );
}

export { Slider };
