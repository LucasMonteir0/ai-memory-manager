// Componentes base no estilo shadcn/ui (Radix + Tailwind + cva).
import * as React from "react";
import { Slot } from "@radix-ui/react-slot";
import * as DialogPrimitive from "@radix-ui/react-dialog";
import { cva, type VariantProps } from "class-variance-authority";
import { X, Loader2 } from "lucide-react";
import { cn, type Tone } from "@/lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-colors disabled:pointer-events-none disabled:opacity-50 [&_svg]:size-4 [&_svg]:shrink-0 cursor-pointer",
  {
    variants: {
      variant: {
        default: "bg-accent text-accent-fg hover:opacity-90",
        outline: "border border-border bg-panel hover:bg-panel-2",
        ghost: "hover:bg-panel-2",
        destructive: "bg-err text-white hover:opacity-90",
        subtle: "bg-panel-2 hover:bg-border",
      },
      size: { default: "h-9 px-3.5", sm: "h-8 px-2.5 text-[13px]", icon: "h-8 w-8" },
    },
    defaultVariants: { variant: "default", size: "default" },
  },
);

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement>, VariantProps<typeof buttonVariants> {
  asChild?: boolean;
  loading?: boolean;
}

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, asChild, loading, children, disabled, ...props }, ref) => {
    const Comp = asChild ? Slot : "button";
    return (
      <Comp ref={ref} className={cn(buttonVariants({ variant, size }), className)} disabled={disabled || loading} {...props}>
        {loading && <Loader2 className="animate-spin" />}
        {children}
      </Comp>
    );
  },
);
Button.displayName = "Button";

export function Card({ className, ...p }: React.HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("rounded-lg border border-border bg-panel", className)} {...p} />;
}
export function CardHeader({ className, ...p }: React.HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("flex items-start justify-between gap-3 border-b border-border px-4 py-3", className)} {...p} />;
}
export function CardTitle({ className, ...p }: React.HTMLAttributes<HTMLHeadingElement>) {
  return <h3 className={cn("text-[15px] font-semibold", className)} {...p} />;
}
export function CardBody({ className, ...p }: React.HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("px-4 py-3", className)} {...p} />;
}

const toneCls: Record<Tone, string> = {
  ok: "bg-ok/12 text-ok border-ok/30",
  warn: "bg-warn/12 text-warn border-warn/30",
  err: "bg-err/12 text-err border-err/30",
  info: "bg-info/12 text-info border-info/30",
  muted: "bg-panel-2 text-muted border-border",
};

export function Badge({ tone = "muted", className, ...p }: React.HTMLAttributes<HTMLSpanElement> & { tone?: Tone }) {
  return (
    <span
      className={cn("inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-xs font-medium whitespace-nowrap", toneCls[tone], className)}
      {...p}
    />
  );
}

export function Alert({ tone = "info", title, children, className }: { tone?: Tone; title?: React.ReactNode; children?: React.ReactNode; className?: string }) {
  return (
    <div role={tone === "err" ? "alert" : "status"} className={cn("rounded-md border px-3 py-2 text-[13px]", toneCls[tone], className)}>
      {title && <div className="font-semibold">{title}</div>}
      {children && <div className="text-fg/90">{children}</div>}
    </div>
  );
}

export const Input = React.forwardRef<HTMLInputElement, React.InputHTMLAttributes<HTMLInputElement>>(({ className, ...p }, ref) => (
  <input
    ref={ref}
    className={cn("h-9 w-full rounded-md border border-border bg-panel px-3 text-sm placeholder:text-muted focus:border-accent outline-none", className)}
    {...p}
  />
));
Input.displayName = "Input";

export function Label({ className, ...p }: React.LabelHTMLAttributes<HTMLLabelElement>) {
  return <label className={cn("text-xs font-medium text-muted", className)} {...p} />;
}

export function Pre({ className, children }: { className?: string; children: React.ReactNode }) {
  return (
    <pre className={cn("scroll-thin max-h-80 overflow-auto rounded-md border border-border bg-panel-2 p-3 font-mono text-xs leading-relaxed whitespace-pre-wrap break-all", className)}>
      {children}
    </pre>
  );
}

export function Kv({ k, v, mono }: { k: React.ReactNode; v: React.ReactNode; mono?: boolean }) {
  return (
    <div className="flex min-w-0 items-baseline justify-between gap-3 py-1 text-[13px]">
      <span className="shrink-0 text-muted">{k}</span>
      <span className={cn("min-w-0 truncate text-right", mono && "font-mono text-xs")} title={typeof v === "string" ? v : undefined}>
        {v}
      </span>
    </div>
  );
}

export function Empty({ icon, title, children }: { icon?: React.ReactNode; title: string; children?: React.ReactNode }) {
  return (
    <div className="flex flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border px-6 py-10 text-center">
      <div className="text-muted">{icon}</div>
      <div className="font-semibold">{title}</div>
      <div className="max-w-md text-[13px] text-muted">{children}</div>
    </div>
  );
}

// Dialog
export const Dialog = DialogPrimitive.Root;
export const DialogTrigger = DialogPrimitive.Trigger;
export const DialogClose = DialogPrimitive.Close;

export function DialogContent({ className, children, title, description, wide }: { className?: string; children: React.ReactNode; title: string; description?: string; wide?: boolean }) {
  return (
    <DialogPrimitive.Portal>
      <DialogPrimitive.Overlay className="fixed inset-0 z-40 bg-black/50" />
      <DialogPrimitive.Content
        className={cn(
          "fixed top-1/2 left-1/2 z-50 flex max-h-[88vh] w-[92vw] -translate-x-1/2 -translate-y-1/2 flex-col rounded-lg border border-border bg-panel shadow-xl",
          wide ? "max-w-4xl" : "max-w-xl",
          className,
        )}
      >
        <div className="flex items-start justify-between gap-3 border-b border-border px-5 py-3">
          <div>
            <DialogPrimitive.Title className="text-base font-semibold">{title}</DialogPrimitive.Title>
            {description ? (
              <DialogPrimitive.Description className="mt-0.5 text-[13px] text-muted">{description}</DialogPrimitive.Description>
            ) : (
              <DialogPrimitive.Description className="sr-only">{title}</DialogPrimitive.Description>
            )}
          </div>
          <DialogPrimitive.Close className="rounded p-1 text-muted hover:bg-panel-2" aria-label="Fechar">
            <X className="size-4" />
          </DialogPrimitive.Close>
        </div>
        <div className="scroll-thin overflow-y-auto px-5 py-4">{children}</div>
      </DialogPrimitive.Content>
    </DialogPrimitive.Portal>
  );
}

export function Section({ title, actions, children, className }: { title: React.ReactNode; actions?: React.ReactNode; children: React.ReactNode; className?: string }) {
  return (
    <section className={cn("space-y-3", className)}>
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="text-[15px] font-semibold">{title}</h2>
        {actions && <div className="flex flex-wrap gap-2">{actions}</div>}
      </div>
      {children}
    </section>
  );
}
