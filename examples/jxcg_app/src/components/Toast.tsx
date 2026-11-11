import type { Toast as ToastType } from "../hooks/useToast";
import { IconAlert, IconCheck, IconClose } from "./icons";
import styles from "./Toast.module.css";

interface Props {
  toasts: ToastType[];
  onRemove: (id: number) => void;
}

const ICONS: Record<string, React.ReactNode> = {
  success: <IconCheck size={15} />,
  error: <IconClose size={15} />,
  warning: <IconAlert size={15} />,
  info: <IconAlert size={15} />,
};

/** 右下角 Toast 队列（对齐 jpcg）。 */
export default function Toast({ toasts, onRemove }: Props) {
  if (toasts.length === 0) return null;
  return (
    <div className={styles.container} role="log" aria-live="polite">
      {toasts.map((t) => (
        <div key={t.id} className={`${styles.toast} ${styles[t.type]}`}>
          <span className={styles.icon}>{ICONS[t.type]}</span>
          <span className={styles.text}>{t.message}</span>
          <button className={styles.close} onClick={() => onRemove(t.id)} aria-label="关闭提示">
            <IconClose size={12} />
          </button>
        </div>
      ))}
    </div>
  );
}
