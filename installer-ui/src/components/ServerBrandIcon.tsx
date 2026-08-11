import { siCaddy, siNginx } from 'simple-icons';
import openLiteSpeedIcon from '../assets/openlitespeed.png';
import type { ServerEngine } from '../types';

export function ServerBrandIcon({ server, size = 46 }: { server: ServerEngine; size?: number }) {
  if (server === 'openlitespeed') {
    return <img src={openLiteSpeedIcon} width={size} height={size} alt="" aria-hidden="true" className="object-contain" />;
  }

  const icon = server === 'nginx' ? siNginx : siCaddy;
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" role="img" aria-label={icon.title} style={{ color: `#${icon.hex}` }}>
      <path fill="currentColor" d={icon.path} />
    </svg>
  );
}
