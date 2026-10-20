interface Props {
  email: string;
  avatar: string | null;
  size?: number;
}

/** The profile picture, or the first letter of the email when none is set. */
export default function Avatar({ email, avatar, size = 32 }: Props) {
  const style = { width: size, height: size, fontSize: size * 0.45 };
  return avatar ? (
    <img className="avatar" src={avatar} alt="" style={style} />
  ) : (
    <span className="avatar avatar-letter" style={style} aria-hidden="true">
      {email.slice(0, 1).toUpperCase()}
    </span>
  );
}
