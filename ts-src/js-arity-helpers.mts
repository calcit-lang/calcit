export let _args_throw = (name: string, expected: number, got: number) => {
  return new Error(`\`${name}\` expected ${expected} params, got ${got}`);
};

export let _args_fewer_throw = (name: string, min: number, got: number) => {
  return new Error(`\`${name}\` expected at least ${min} params, got ${got}`);
};

export let _args_between_throw = (name: string, min: number, max: number, got: number) => {
  return new Error(`\`${name}\` expected ${min}-${max} params, got ${got}`);
};
