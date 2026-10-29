import {
  Add,
  ArrowLeft2,
  ArrowRight2,
  Chart1,
  CloseCircle,
  Copy,
  Edit2,
  Eye,
  EyeSlash,
  InfoCircle,
  Key,
  Logout,
  Refresh,
  Setting2,
  TickCircle,
  Trash,
  type Icon as IconsaxIcon,
} from "iconsax-react";

/**
 * App icon set — iconsax Linear, with a locally drawn usage gauge.
 * Linear is the 1.5px outline tier: light, minimal lines that match the
 * quiet menubar-panel aesthetic. `color` is always currentColor so icons
 * inherit text color. Default size is 20 (up from 18) because outlines
 * read smaller than fills at the same box.
 */
interface IconProps {
  size?: number | string;
  className?: string;
}

function linear(Icon: IconsaxIcon) {
  return function LinearIcon({ size = 20, className }: IconProps) {
    return (
      <Icon
        variant="Linear"
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

export const SettingIcon = linear(Setting2);
export const PrevIcon = linear(ArrowLeft2);
export const NextIcon = linear(ArrowRight2);
export const TrashIcon = linear(Trash);
export const InfoCircleIcon = linear(InfoCircle);

/** Locally drawn quota gauge, matching the app's 1.5px Linear outlines. */
export function UsageIcon({ size = 20, className }: IconProps) {
  return (
    <svg
      className={`ak-icon${className ? ` ${className}` : ""}`}
      data-icon="usage-gauge"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M5.6 19a9 9 0 1 1 12.8 0H5.6Z" />
      <path d="M5 12h2m10 0h2M12 5v2M7 7l1.4 1.4M16 8l-3 4" />
      <circle cx="12" cy="13" r="1.5" />
    </svg>
  );
}
export const ChartIcon = linear(Chart1);
export const LogoutIcon = linear(Logout);
export const CloseIcon = linear(CloseCircle);
export const AddIcon = linear(Add);
export const SaveIcon = linear(TickCircle);
export const CopyIcon = linear(Copy);
export const EyeIcon = linear(Eye);
export const EyeOffIcon = linear(EyeSlash);
export const KeyIcon = linear(Key);
export const TickIcon = linear(TickCircle);
export const RefreshIcon = linear(Refresh);

/** Two-way account switch, distinct from the data refresh action. */
export function AccountSwitchIcon({ size = 20 }: IconProps) {
  return <svg className="ak-icon" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M4 8h15m-4-4 4 4-4 4M20 16H5m4-4-4 4 4 4" /></svg>;
}

/** Rename an account without changing its login. */
export const EditIcon = linear(Edit2);
