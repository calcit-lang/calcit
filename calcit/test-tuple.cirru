
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-tuple) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn |test-tuple.main/main!) (:mode :native) (:reload-fn |test-tuple.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |test-tuple.main $ %{} :FileEntry
      :defs $ {}
        |Result $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defenum Result (:ok :number) (:err :string)
          :examples $ []
        |main! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn main! () (log-title "|Testing tuple")
              assert= (:: :parts |1 |23)
                tag-match (destruct-str |123)
                  (:none) (:: :empty)
                  (:some s0 ss) (:: :parts s0 ss)
              assert= (:: :empty)
                tag-match (destruct-str |)
                  (:none) (:: :empty)
                  (:some s0 ss) (:: :parts s0 ss)
              assert=
                :: :parts 1 $ [] 2 3
                tag-match
                  destruct-list $ [] 1 2 3
                  (:none) (:: :empty)
                  (:some l0 ls) (:: :parts l0 ls)
              assert= (:: :empty)
                tag-match
                  destruct-list $ []
                  (:none) (:: :empty)
                  (:some l0 ls) (:: :parts l0 ls)
              assert= (:: :parts true 2)
                tag-match
                  destruct-set $ #{} 1 2 3
                  (:none) (:: :empty)
                  (:some l0 ls)
                    :: :parts (number? l0) (count ls)
              assert= (:: :empty)
                tag-match
                  destruct-set $ #{}
                  (:none) (:: :empty)
                  (:some l0 ls)
                    :: :parts (number? l0) (count ls)
              assert= (:: :parts true true 1)
                tag-match
                  destruct-map $ &{} :a 1 :b 2
                  (:none) (:: :empty)
                  (:some k0 v0 ms)
                    :: :parts (tag? k0) (number? v0) (count ms)
              assert= (:: :empty)
                tag-match
                  destruct-map $ &{}
                  (:none) (:: :empty)
                  (:some k0 v0 ms)
                    :: :parts $ count ms
              assert= 1 $ try-size (:: :dyn)
              assert= 2 $ try-size (:: :dyn 1)
              assert= 3 $ try-size (:: :dyn 1 2)
              assert= 4 $ try-size (:: :dyn 1 2 3)
              assert= :many $ try-size (:: :dyn 1 2 3 4)
              assert= :many $ try-size (:: :dyn 1 2 3 4 5)
              let
                  ok $ %:: Result :ok 1
                assert= :enum $ type-of (&tuple:enum ok)
                assert= "|(%:: :ok 1 (:enum Result))" $ str ok
                assert= true $ &tuple:enum-has-variant? Result :ok
                assert= 1 $ &tuple:enum-variant-arity Result :ok
                assert= nil $ &tuple:validate-enum ok :ok
              let
                  plain $ :: :plain 1
                assert= nil $ &tuple:enum plain
          :examples $ []
        |try-size $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn try-size (x)
              tag-match x
                (:dyn) 1
                (:dyn x) 2
                (:dyn x y) 3
                (:dyn x y z) 4
                _ :many
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ [] :dynamic
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns test-tuple.main $ :require
            util.core :refer $ log-title
