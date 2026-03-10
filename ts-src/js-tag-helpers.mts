import { CalcitTag, newTag } from "./calcit-data.mjs";

let _tag_cache: Record<string, CalcitTag> = {};

export let init_tags = (arr: string[]) => {
  let tags: Record<string, CalcitTag> = {};
  for (let idx = 0; idx < arr.length; idx++) {
    let name = arr[idx];
    let item = _tag_cache[name];
    if (item === undefined) {
      item = newTag(name);
      _tag_cache[name] = item;
    }
    tags[name] = item;
  }
  return tags;
};
