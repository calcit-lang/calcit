
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.applied-data $ %{} 'FileEntry
      :defs $ {}
        'DbA $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct DbA (:value 'Number)
          :examples $ []
          :schema $ quote StructDef
        'DbB $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct DbB (:value 'String)
          :examples $ []
          :schema $ quote StructDef
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.applied-data
    'app.applied-model $ %{} 'FileEntry
      :defs $ {}
        'ClosedLike $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct ClosedLike ([] 'Db) (:base 'Db) (:db 'Db)
          :examples $ []
          :schema $ quote StructDef
        'OuterLike $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct OuterLike ([] 'Db)
            :reel $ quote $ app.applied-model/ReelLike Db
          :examples $ []
          :schema $ quote StructDef
        'PairLike $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct PairLike ([] 'Key 'Payload) (:key 'Key) (:payload 'Payload)
          :examples $ []
          :schema $ quote StructDef
        'ReelAlias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def ReelAlias ReelLike
          :examples $ []
          :schema $ quote StructDef
        'ReelLike $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct ReelLike ([] 'Db) (:base 'Db) (:db 'Db)
            :records $ quote $ List (List Dynamic)
            :merged? 'Bool
          :examples $ []
          :schema $ quote StructDef
        'keep-db $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn keep-db (reel) (:db reel)
          :examples $ []
          :schema $ quote $ Fn (app.applied-model/ReelLike Db) (-> Db) (:generics Db)
        'step $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn step (reel updater)
            ReelLike :base (:base reel) :db
              updater $ :db reel
              , :records (:records reel) :merged? $ :merged? reel
          :examples $ []
          :schema $ quote $ Fn (app.applied-model/ReelLike Db)
            Fn Db $ -> Db
            -> $ app.applied-model/ReelLike Db
            :generics Db
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.applied-model
          :require $ app.applied-data :refer $ DbA DbB
    'app.applied-reader $ %{} 'FileEntry
      :defs $ {}
        'append-b $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn append-b (db)
            data/DbB :value $ str (:value db) |!
          :examples $ []
          :schema $ quote $ Fn data/DbB (-> data/DbB)
        'bump-a $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bump-a (db)
            data/DbA :value $ .inc $ :value db
          :examples $ []
          :schema $ quote $ Fn data/DbA (-> data/DbA)
        'make-pair $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn make-pair (key payload) (model/PairLike :key key :payload payload)
          :examples $ []
          :schema $ quote $ Fn data/DbA Dynamic
            -> $ model/PairLike data/DbA Dynamic
        'read-a $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-a (reel)
            let
                alias $ model/keep-db reel
              .inc $ :value alias
          :examples $ []
          :schema $ quote $ Fn (model/ReelLike data/DbA) (-> Number)
          :tests $ [] $ %{} 'TestEntry (:name |two-db-updaters-preserve-history)
            :code $ quote $ let
                a $ model/ReelLike :base (data/DbA :value 1) :db (data/DbA :value 2) :records
                  [] $ [] 1 |open
                  , :merged? false
                b $ model/ReelLike :base (data/DbB :value |base) :db (data/DbB :value |two) :records ([]) :merged? true
              assert= 4 $ read-a $ model/step a bump-a
              assert= 4 $ read-b $ model/step b append-b
              assert= 2 $ :value $ model/keep-db a
              assert= |two $ :value $ model/keep-db b
              assert= 6 $ read-a $ with-a a (data/DbA :value 5)
              assert=
                [] $ [] 1 |open
                :records $ model/step a bump-a
            :tags $ #{} :applied-struct-evidence
        'read-b $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-b (reel)
            .len $ :value $ :db reel
          :examples $ []
          :schema $ quote $ Fn (model/ReelLike data/DbB) (-> Number)
        'read-closed-a $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-closed-a (reel)
            .inc $ :value $ :db reel
          :examples $ []
          :schema $ quote $ Fn (model/ClosedLike data/DbA) (-> Number)
          :tests $ [] $ %{} 'TestEntry (:name |closed-construction-and-alias)
            :code $ quote $ let
                reel $ model/ClosedLike :base (data/DbA :value 0) :db $ data/DbA :value 2
                alias reel
              assert= 3 $ read-closed-a alias
              assert= 5 $ read-closed-a $ .assoc alias :db (data/DbA :value 4)
            :tags $ #{} :applied-struct-closed :applied-struct-evidence
        'read-key $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-key (pair)
            .inc $ :value $ :key pair
          :examples $ []
          :schema $ quote $ Fn (model/PairLike data/DbA Dynamic) (-> Number)
          :tests $ [] $ %{} 'TestEntry (:name |independent-generic-open-payload)
            :code $ quote $ assert= 5
              read-key $ make-pair (data/DbA :value 4) ([] 1 |open)
            :tags $ #{} :applied-struct-evidence
        'read-nested-a $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-nested-a (outer)
            read-a $ :reel outer
          :examples $ []
          :schema $ quote $ Fn (model/OuterLike data/DbA) (-> Number)
          :tests $ [] $ %{} 'TestEntry (:name |aliases-and-nested-fields)
            :code $ quote $ let
                alias $ model/ReelAlias :merged? false :records ([]) :db (data/DbA :value 8) :base $ data/DbA :value 0
                outer $ model/OuterLike :reel alias
              assert= 9 $ read-nested-a outer
              assert= 10 $ read-a $ model/step alias bump-a
            :tags $ #{} :applied-struct-evidence
        'with-a $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn with-a (reel db) (.assoc reel :db db)
          :examples $ []
          :schema $ quote $ Fn (model/ReelLike data/DbA) data/DbA
            -> $ model/ReelLike data/DbA
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.applied-reader
          :require (app.applied-data :as data) (app.applied-model :as model)
    'app.checked-write $ %{} 'FileEntry
      :defs $ {}
        'CoreNominalBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct CoreNominalBox
            :maybe $ quote $ Option Number
            :qualified $ quote $ calcit.core/Option Number
            :outcome $ quote $ Result Number String
          :examples $ []
          :schema $ quote StructDef
        'NilBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct NilBox (:none 'Nil)
            :host $ quote $ JsNullish Number
          :examples $ []
          :schema $ quote StructDef
        'NumBox $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct NumBox (:small 'Int8) (:tiny 'Int8) (:wide 'UInt16) (:single 'Float32)
          :examples $ []
          :schema $ quote StructDef
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.checked-write
    'app.foreign-nominal $ %{} 'FileEntry
      :defs $ {}
        'Option $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Option (:none) (:some 'Number)
          :examples $ []
          :schema $ quote EnumDef
        'Result $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Result (:ok 'Number) (:err 'String)
          :examples $ []
          :schema $ quote EnumDef
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.foreign-nominal
    'app.main $ %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
