import React from 'react';
import mage from 'art/novicemage_single.png';
import ranger from 'art/noviceranger_single.png';
import warrior from 'art/novicewarrior_single.png';
import { HERO_PORTRAITS, portraitUrl } from './portraitCatalog';

interface HeroCreationPanelProps {
  heroName: string;
  selectedClass: string;
  selectedPortrait: string;
  classImageSize?: number;
  isHeroNameEmpty: boolean;
  isClassMissing: boolean;
  inappropriateName: boolean;
  takenName: boolean;
  onHeroNameChange: (event: React.ChangeEvent<HTMLInputElement>) => void;
  onClassSelect: (className: string) => void;
  onPortraitSelect: (portrait: string) => void;
  onCreate: () => void;
  onShowLogin: (event: React.MouseEvent) => void;
}

const CLASS_OPTIONS = [
  {
    name: 'Warrior',
    image: warrior,
    title: 'Safest adjacent fighter; wins by bracing, stunning, and sustaining pressure.',
  },
  {
    name: 'Ranger',
    image: ranger,
    title: 'Fastest scout and kiter; wins by vision, bow range, and disengaging.',
  },
  {
    name: 'Mage',
    image: mage,
    title: 'Fragile mana caster; wins by burst damage and temporary magical protection.',
  },
];

const panelStyle: React.CSSProperties = {
  position: 'fixed',
  top: '50%',
  left: '50%',
  transform: 'translate(-50%, -50%)',
  width: 'min(620px, calc(100vw - 24px))',
  maxHeight: 'calc(100vh - 24px)',
  overflowY: 'auto',
  boxSizing: 'border-box',
  padding: '24px 28px',
  border: '1px solid #a98b58',
  borderRadius: '8px',
  background: 'linear-gradient(180deg, rgba(29, 31, 34, 0.98), rgba(12, 14, 16, 0.98))',
  boxShadow: '0 18px 60px rgba(0, 0, 0, 0.75), inset 0 0 30px rgba(169, 139, 88, 0.08)',
  color: '#fffff0',
  fontFamily: 'Cinzel, serif',
  zIndex: 20,
};

const choiceRowStyle: React.CSSProperties = {
  display: 'flex',
  justifyContent: 'center',
  gap: '14px',
  flexWrap: 'wrap',
};

export default function HeroCreationPanel(props: HeroCreationPanelProps) {
  const classImageSize = props.classImageSize ?? 64;
  const classChoiceWidth = classImageSize + 24;

  const error = props.inappropriateName
    ? 'Inappropriate name'
    : props.takenName
      ? 'Name already taken'
      : props.isHeroNameEmpty
        ? 'Enter a hero name'
        : props.isClassMissing
          ? 'Choose a hero class'
          : '';

  return (
    <section style={panelStyle} aria-labelledby="create-hero-title">
      <h2 id="create-hero-title" style={{ margin: '0 0 18px', textAlign: 'center', letterSpacing: '0.06em' }}>
        Create Your Hero
      </h2>

      <label style={{ display: 'block', marginBottom: '18px', fontSize: '14px' }}>
        Hero&apos;s Name
        <input
          type="text"
          autoFocus
          value={props.heroName}
          onChange={props.onHeroNameChange}
          style={{
            display: 'block',
            width: '100%',
            height: '38px',
            marginTop: '7px',
            boxSizing: 'border-box',
            padding: '0 12px',
            border: props.isHeroNameEmpty ? '1px solid #bd4f4f' : '1px solid #596069',
            borderRadius: '4px',
            background: '#24282d',
            color: '#f1eadc',
            fontFamily: 'Verdana, sans-serif',
          }}
        />
      </label>

      <div style={{ marginBottom: '10px', fontSize: '14px' }}>Hero&apos;s Class</div>
      <div style={choiceRowStyle}>
        {CLASS_OPTIONS.map(option => {
          const selected = props.selectedClass === option.name;
          return (
            <button
              type="button"
              key={option.name}
              onClick={() => props.onClassSelect(option.name)}
              title={`${option.name}: ${option.title}`}
              aria-pressed={selected}
              style={{
                width: `${classChoiceWidth}px`,
                minHeight: `${classImageSize + 40}px`,
                boxSizing: 'border-box',
                padding: '10px',
                border: selected ? '2px solid #d9b46d' : '1px solid #596069',
                borderRadius: '5px',
                background: selected ? 'rgba(169, 139, 88, 0.22)' : '#20242a',
                color: '#fffff0',
                cursor: 'pointer',
                fontFamily: 'Cinzel, serif',
              }}
            >
              <img
                src={option.image}
                alt=""
                style={{
                  display: 'block',
                  width: `${classImageSize}px`,
                  height: `${classImageSize}px`,
                  margin: '0 auto',
                  imageRendering: 'pixelated',
                  objectFit: 'contain',
                }}
              />
              <span style={{ display: 'block', marginTop: '4px' }}>{option.name}</span>
            </button>
          );
        })}
      </div>

      <div style={{ margin: '20px 0 10px', fontSize: '14px' }}>Choose a Portrait</div>
      <div style={choiceRowStyle}>
        {HERO_PORTRAITS.map((portrait, index) => {
          const selected = props.selectedPortrait === portrait;
          return (
            <button
              type="button"
              key={portrait}
              onClick={() => props.onPortraitSelect(portrait)}
              title={`Portrait ${index + 1}`}
              aria-label={`Choose portrait ${index + 1}`}
              aria-pressed={selected}
              style={{
                width: '84px',
                height: '84px',
                padding: '3px',
                border: selected ? '3px solid #d9b46d' : '1px solid #596069',
                borderRadius: '50%',
                background: '#15181c',
                cursor: 'pointer',
                overflow: 'hidden',
              }}
            >
              <img
                src={portraitUrl(portrait) || ''}
                alt=""
                style={{ width: '100%', height: '100%', borderRadius: '50%', objectFit: 'cover' }}
              />
            </button>
          );
        })}
      </div>

      <div style={{ minHeight: '20px', marginTop: '10px', color: '#dd7373', fontFamily: 'Verdana, sans-serif', fontSize: '12px', textAlign: 'center' }} role="alert">
        {error}
      </div>

      <button
        type="button"
        onClick={props.onCreate}
        style={{
          display: 'block',
          width: '220px',
          height: '42px',
          margin: '4px auto 0',
          border: '1px solid #c7a667',
          borderRadius: '4px',
          background: 'linear-gradient(#725c37, #4d3c23)',
          color: '#fffff0',
          cursor: 'pointer',
          fontFamily: 'Cinzel, serif',
          fontSize: '15px',
        }}
      >
        Begin Your Journey
      </button>

      <button
        type="button"
        onClick={props.onShowLogin}
        style={{ display: 'block', margin: '14px auto 0', border: 0, background: 'transparent', color: '#b4bcc4', cursor: 'pointer' }}
      >
        Already have an account? Log in
      </button>
    </section>
  );
}
