import {
  motion,
  useReducedMotion,
  type HTMLMotionProps,
  type Variants,
} from "motion/react";

// One shared easing/rhythm so every animation in the app feels consistent.
const EASE = [0.22, 1, 0.36, 1] as const;

const container: Variants = {
  hidden: {},
  show: { transition: { staggerChildren: 0.045 } },
};
const containerReduced: Variants = {
  hidden: {},
  show: { transition: { staggerChildren: 0 } },
};

const item: Variants = {
  hidden: { opacity: 0, y: 8 },
  show: { opacity: 1, y: 0, transition: { duration: 0.3, ease: EASE } },
};
const itemReduced: Variants = {
  hidden: { opacity: 0 },
  show: { opacity: 1, transition: { duration: 0.2 } },
};

/** Container that reveals its `<StaggerItem>` children one after another. */
export function Stagger(props: HTMLMotionProps<"div">) {
  const reduce = useReducedMotion();
  return (
    <motion.div
      initial="hidden"
      animate="show"
      variants={reduce ? containerReduced : container}
      {...props}
    />
  );
}

/** A single fade-and-rise item; use inside a `<Stagger>`. */
export function StaggerItem(props: HTMLMotionProps<"div">) {
  const reduce = useReducedMotion();
  return <motion.div variants={reduce ? itemReduced : item} {...props} />;
}
