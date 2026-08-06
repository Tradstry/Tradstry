import { useState, type ReactNode } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";

const sizes = {
  sm: "sm:max-w-sm",
  md: "sm:max-w-lg",
  lg: "sm:max-w-2xl",
  xl: "sm:max-w-3xl",
} as const;

type ModalProps = {
  /** The element that opens the modal — must forward props/ref (a shadcn Button
   *  or plain button; Radix clones it via `asChild`). */
  trigger: ReactNode;
  title?: string;
  description?: string;
  /** Optional leading icon shown in a rounded chip before the title. */
  icon?: ReactNode;
  size?: keyof typeof sizes;
  /** Content, or a render function receiving `close` to dismiss the modal. */
  children: ReactNode | ((close: () => void) => ReactNode);
};

/**
 * App-wide modal: a liquid-glass panel (translucent + backdrop-blur, see
 * ui/dialog) over a blurred scrim, with a scale+fade entrance, focus trap,
 * escape/click-outside dismiss, and a built-in close button. Built on the
 * shadcn Dialog so it matches the rest of the design system.
 */
export function Modal({
  trigger,
  title,
  description,
  icon,
  size = "md",
  children,
}: ModalProps) {
  const [open, setOpen] = useState(false);
  const close = () => setOpen(false);

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>{trigger}</DialogTrigger>
      <DialogContent className={`${sizes[size]} max-h-[85vh] overflow-y-auto`}>
        {title ? (
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              {icon ? (
                <span className="flex size-7 items-center justify-center rounded-md bg-muted text-muted-foreground">
                  {icon}
                </span>
              ) : null}
              {title}
            </DialogTitle>
            {description ? (
              <DialogDescription>{description}</DialogDescription>
            ) : null}
          </DialogHeader>
        ) : (
          // Radix requires a title for a11y; hide it when the caller has none.
          <DialogTitle className="sr-only">Dialog</DialogTitle>
        )}
        {typeof children === "function" ? children(close) : children}
      </DialogContent>
    </Dialog>
  );
}
