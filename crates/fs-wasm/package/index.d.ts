export interface FeatureParameter {
  name: string;
  caption: string;
  description: string;
  group: string;
  type: 'number' | 'bool' | 'string';
  value: number | boolean | string;
  units: 'mm' | '°' | '';
  control: {
    kind: 'spinbox' | 'checkbox' | 'text' | 'dropdown';
    min?: number;
    max?: number;
    step?: number;
    options?: { value: string; label: string }[];
  };
  condition?: { parameter: string; value: number | boolean | string };
}

export interface FeatureParameters {
  feature: string;
  features: string[];
  params: FeatureParameter[];
}

export interface BuiltFeature {
  stl: Uint8Array;
  step: Uint8Array;
  log: string;
}

/** A feature built and held by the engine, until its next `model` or `build`. */
export interface FeatureModel {
  /** What the feature printed. */
  output: string;
  /** A binary STL in millimetres, meshed to within `tolerance` mm (0.01 if left out). */
  mesh(tolerance?: number): Uint8Array;
  step(): Uint8Array;
  /** Each part's volume, a line each. */
  volumes(): string;
}

export interface Engine {
  parameters(source: string, feature?: string): Promise<FeatureParameters>;
  build(source: string, values?: Record<string, number | boolean | string>, feature?: string): Promise<BuiltFeature>;
  model(source: string, values?: Record<string, number | boolean | string>, feature?: string): Promise<FeatureModel>;
  /** The memory the engine has taken, in bytes. */
  memory(): number;
}

export const FONTS: string[];
export function fontsUsed(source: string): string[];
export function createEngine(options?: {
  locateFile?: (name: string) => string;
  loadFont?: (name: string) => Promise<ArrayBuffer | Uint8Array>;
  print?: (line: string) => void;
}): Promise<Engine>;
