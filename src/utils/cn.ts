import clsx, { type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
/** Combine conditional classes and merge conflicting Tailwind utilities. */
export const cn = (...inputs: ClassValue[]): string => twMerge(clsx(inputs));
