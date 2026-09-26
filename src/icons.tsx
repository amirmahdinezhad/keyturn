import {
  Add,
  ArrowLeft2,
  ArrowRight2,
  Chart1,
  CloseCircle,
  Copy,
  Eye,
  EyeSlash,
  InfoCircle,
  Key,
  Logout,
  Refresh,
  Setting2,
  TickCircle,
  Speedometer,
  Trash,
  type Icon as IconsaxIcon,
} from "iconsax-react";

/**
 * App icon set — iconsax, Bold style only.
 * Bold is the filled tier: solid shapes that stay legible at 16–20px in the
 * menubar panel. Linear (1.5px outline) reads thin and washed-out at these
 * sizes, so it is never used here. `color` is always currentColor so icons
 * inherit text color.
 */
interface IconProps {
  size?: number | string;
  className?: string;
}

function bold(Icon: IconsaxIcon) {
  return function BoldIcon({ size = 18, className }: IconProps) {
    return (
      <Icon
        variant="Bold"
        size={size}
        color="currentColor"
        className={`ak-icon${className ? ` ${className}` : ""}`}
      />
    );
  };
}

/** Determinate-feel spinner for busy states — inherits text color. */
export function SpinnerIcon({ size = 16, className }: IconProps) {
  return (
    <svg
      className={`ak-icon ak-spin${className ? ` ${className}` : ""}`}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
    >
      <circle
        cx="12"
        cy="12"
        r="9"
        stroke="currentColor"
        strokeOpacity="0.25"
        strokeWidth="3"
      />
      <path
        d="M21 12a9 9 0 0 0-9-9"
        stroke="currentColor"
        strokeWidth="3"
        strokeLinecap="round"
      />
    </svg>
  );
}

export const SettingIcon = bold(Setting2);
export const PrevIcon = bold(ArrowLeft2);
export const NextIcon = bold(ArrowRight2);
export const TrashIcon = bold(Trash);
export const InfoCircleIcon = bold(InfoCircle);
export const UsageIcon = bold(Speedometer);
export const ChartIcon = bold(Chart1);
export const LogoutIcon = bold(Logout);
export const CloseIcon = bold(CloseCircle);
export const AddIcon = bold(Add);
export const SaveIcon = bold(TickCircle);
export const CopyIcon = bold(Copy);
export const EyeIcon = bold(Eye);
export const EyeOffIcon = bold(EyeSlash);
export const KeyIcon = bold(Key);
export const TickIcon = bold(TickCircle);
export const RefreshIcon = bold(Refresh);
