import styles from "./StatusBar.module.css";

interface Props {
  message: string;
  fuwuqi?: string;
  banben?: string;
  mock?: boolean;
}

/** 底部状态栏：左侧运行消息，右侧当前服务器/数据版本/预览标识。 */
export default function StatusBar({ message, fuwuqi, banben, mock }: Props) {
  return (
    <footer className={styles.bar} role="status">
      <span className={styles.dot} />
      <span className={styles.message}>{message}</span>
      <span className={styles.spacer} />
      {fuwuqi && <span className={styles.meta}>{fuwuqi}</span>}
      {banben && (
        <span className={styles.meta} title={banben}>
          {banben}
        </span>
      )}
      {mock && <span className={styles.mock}>浏览器预览 mock</span>}
    </footer>
  );
}
