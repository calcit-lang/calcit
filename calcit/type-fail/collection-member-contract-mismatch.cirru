
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --full` first. Manual edits must follow format and schema conventions, then run `calcit edit format`.") (:package |type-fail-collection-member-contract)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'type-fail-collection-member-contract.main/main!) (:mode :native) (:reload-fn 'type-fail-collection-member-contract.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'type-fail-collection-member-contract.main $ %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc "|Reject mismatched collection lookup, membership, key, and association arguments")
          :code $ quote
            defn main! ()
              get ([] 1 2) :bad
              get (&{} :a 1) 0
              includes? (#{} 1 2) :bad
              includes? (&{} :a 1) :a
              includes? |abc 1
              contains? ([] 1 2) :bad
              contains? (&{} :a 1) 0
              contains? (#{} 1 2) :bad
              assoc ([] 1 2) :bad 3
              assoc ([] 1 2) 0 :bad
              assoc (&{} :a 1) 0 2
              assoc (&{} :a 1) :a :bad
              contains? (:: :ok 1) :bad
              assoc (:: :ok 1) :bad 2
              dissoc ([] 1 2) :bad
              dissoc (&{} :a 1) 0
              dissoc (&{} :a 1 :b 2) :a 0
              &map:dissoc (&{} :a 1 :b 2) :a 0
              &list:concat ([] 1) ([] 2) ([] :bad)
              &merge (&{} :a 1) (&{} :b 2) (&{} :c :bad)
              filter ([] 1 2) inc
              filter (#{} 1 2) inc
              filter (&{} :a 1) inc
              any? ([] 1 2) inc
              every? (&{} :a 1) inc
              each ([] |a |b) inc
              map ([] |a |b) inc
              map (#{} |a |b) inc
              map (&{} :a 1) inc
              foldl ([] |a |b) 0 +
              foldl (#{} |a |b) 0 +
              foldl (&{} :a 1) 0 +
              reduce (#{} |a |b) 0 +
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler")
          :code $ quote
            defn reload! () nil
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
      :ns $ %{} 'NsEntry (:doc "|Namespace for collection member contract mismatches")
        :code $ quote (ns type-fail-collection-member-contract.main)
