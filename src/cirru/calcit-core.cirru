
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |calcit) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'calcit.core/println!) (:mode :native) (:reload-fn 'calcit.core/println!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |calcit.core $ %{} :FileEntry
      :defs $ {}
        |#{} $ %{} :CodeEntry (:doc "|internal function for creating sets\nSyntax: (#{} & elements)\nParams: elements (any, variadic)\nReturns: set\nCreates new set from provided elements")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:rest 'T)
              :args $ []
              :generics $ [] 'T
              :return $ :: 'Set 'T
          :tags $ #{} :builtin :internal
        |%:: $ %{} :CodeEntry (:doc "|internal function for creating enum tuples\nSyntax: (%:: enum tag & values)\nParams: enum (record/enum), tag (tag), values (any, variable number)\nReturns: tuple with enum metadata\nCreates a tagged tuple that carries enum metadata for validation (use &tuple:impl-traits to attach impls)")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Tuple)
              :args $ [] 'Enum 'Tag
          :tags $ #{} :builtin :internal
        |%<- $ %{} :CodeEntry (:doc "|pass value as `%` into several expressions, in reversed order")
          :code $ quote
            defmacro %<- (& xs)
              if (&list:empty? xs) (raise "|%<- expects at least 1 expression")
              quasiquote $ ->%
                ~@ $ reverse xs
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :macro
        |%err $ %{} :CodeEntry (:doc "|Create Err variant of Result")
          :code $ quote
            defn %err (message) (%:: Result :err message)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'E
              :generics $ [] 'T 'E
              :return $ :: 'Result 'T 'E
        |%none $ %{} :CodeEntry (:doc "|Create None variant of Option")
          :code $ quote
            defn %none () $ %:: Option :none
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ []
              :generics $ [] 'T
              :return $ :: 'Option 'T
        |%ok $ %{} :CodeEntry (:doc "|Create Ok variant of Result")
          :code $ quote
            defn %ok (value) (%:: Result :ok value)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'T
              :generics $ [] 'T 'E
              :return $ :: 'Result 'T 'E
        |%some $ %{} :CodeEntry (:doc "|Create Some variant of Option")
          :code $ quote
            defn %some (value) (%:: Option :some value)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'T
              :generics $ [] 'T
              :return $ :: 'Option 'T
        |%{} $ %{} :CodeEntry (:doc "|Macro for constructing struct-based records\nSyntax: (%{} StructName & field-value-pairs)\nParams: StructName (struct from defstruct), field-value-pairs (key-value list pairs, variadic)\nReturns: record")
          :code $ quote
            defmacro %{} (R & xs)
              if
                not $ and (list? xs) (every? xs list?)
                raise $ str-spaced "|%{} expects field entries in list, got:" xs
              &let
                args $ &list:concat & xs
                quasiquote $ &%{} ~R ~@args
          :examples $ []
            quote $ let
                Point $ defstruct Point (:x 'Number) (:y 'Number)
                rec $ %{} Point ([] :x 1) ([] :y 2)
              assert= 1 $ :x rec
          :schema $ :: 'Macro
            {} (:return 'Record)
              :args $ [] 'Struct
          :tags $ #{} :macro
        |%{}? $ %{} :CodeEntry (:doc "|Partial record constructor — allows omitting optional fields\nOmitted fields default to nil when using a struct prototype.\nSyntax: (%{}? R & field-value-pairs)\nParams: R (struct), field-value-pairs (key-value list pairs, variadic)\nReturns: record")
          :code $ quote
            defmacro %{}? (R & xs)
              if
                not $ and (list? xs) (every? xs list?)
                raise $ str-spaced "|%{}? expects field entries in list, got:" xs
              &let
                args $ &list:concat & xs
                quasiquote $ &%{}? ~R ~@args
          :examples $ []
            quote $ let
                Point $ defstruct Point (:x 'Number) (:y 'Number)
                p $ %{}? Point (:x 1)
              assert= nil $ :y p
          :schema $ :: 'Macro
            {} (:return 'Record)
              :args $ [] 'Struct
          :tags $ #{} :macro
        |& $ %{} :CodeEntry (:doc "|internal syntax for spreading in function definition and call\nSyntax: (& rest-args) in params or (f & args) in calls\nParams: varies based on context\nReturns: varies based on context\nMarks rest parameters or argument spreading")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal :syntax
        |&%{} $ %{} :CodeEntry (:doc "|internal function for native record creation\nSyntax: (&%{} name & key-value-pairs)\nParams: name (keyword), key-value-pairs (any, variadic)\nReturns: record\nCreates native record with name and fields")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Record)
              :args $ [] 'Struct
          :tags $ #{} :builtin :internal
        |&%{}? $ %{} :CodeEntry (:doc "|internal function for partial record construction\nSyntax: (&%{}? proto & key-value-pairs)\nParams: proto (struct), key-value-pairs (any, variadic)\nReturns: record\nMissing fields default to nil.")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Record)
              :args $ [] 'Struct
          :tags $ #{} :builtin :internal
        |&* $ %{} :CodeEntry (:doc "|internal function for multiplication\nSyntax: (&* a b)\nParams: a (number), b (number)\nReturns: number\nMultiplies two numbers together, supports integers and floats")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number 'Number
          :tags $ #{} :builtin :internal
        |&+ $ %{} :CodeEntry (:doc "|internal function for addition\nSyntax: (&+ a b)\nParams: a (number), b (number)\nReturns: number\nAdds two numbers together, supports integers and floats")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number 'Number
          :tags $ #{} :builtin :internal
        |&- $ %{} :CodeEntry (:doc "|internal function for subtraction\nSyntax: (&- a b)\nParams: a (number), b (number)\nReturns: number\nSubtracts second number from first, supports integers and floats")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number 'Number
          :tags $ #{} :builtin :internal
        |&/ $ %{} :CodeEntry (:doc "|internal function for division\nSyntax: (&/ a b)\nParams: a (number), b (number)\nReturns: number\nDivides first number by second, returns float result")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number 'Number
          :tags $ #{} :builtin :internal
        |&< $ %{} :CodeEntry (:doc "|internal function for less than comparison\nSyntax: (&< a b & values)\nParams: a (number), b (number), values (number, variadic)\nReturns: boolean\nReturns true if values are in ascending order")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Number 'Number
          :tags $ #{} :builtin :internal
        |&<= $ %{} :CodeEntry (:doc "|Less than or equal comparison for two values")
          :code $ quote
            defn &<= (a b)
              assert "|expects numbers for &<=" $ if (number? a) (number? b)
              if (&< a b) true $ &= a b
          :examples $ []
            quote $ assert= true (&<= 3 5)
            quote $ assert= true (&<= 5 5)
            quote $ assert= false (&<= 5 3)
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Number 'Number
          :tags $ #{} :internal
        |&= $ %{} :CodeEntry (:doc "|internal function for equality comparison\nSyntax: (&= a b & values)\nParams: a (any), b (any), values (any, variadic)\nReturns: boolean\nReturns true if all values are equal using deep comparison")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Dynamic 'Dynamic
          :tags $ #{} :builtin :internal
        |&> $ %{} :CodeEntry (:doc "|internal function for greater than comparison\nSyntax: (&> a b & values)\nParams: a (number), b (number), values (number, variadic)\nReturns: boolean\nReturns true if values are in descending order")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Number 'Number
          :tags $ #{} :builtin :internal
        |&>= $ %{} :CodeEntry (:doc "|Greater than or equal comparison for two values")
          :code $ quote
            defn &>= (a b)
              assert "|expects numbers for &>=" $ if (number? a) (number? b)
              if (&> a b) true $ &= a b
          :examples $ []
            quote $ assert= true (&>= 5 3)
            quote $ assert= true (&>= 5 5)
            quote $ assert= false (&>= 3 5)
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Number 'Number
          :tags $ #{} :internal
        |&atom:deref $ %{} :CodeEntry (:doc "|internal function for dereferencing atoms\nSyntax: (&atom:deref atom)\nParams: atom (atom)\nReturns: any\nReturns current value of atom")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'T)
              :args $ [] (:: 'Ref 'T)
              :generics $ [] 'T
          :tags $ #{} :builtin :internal :state
        |&buf-list:concat $ %{} :CodeEntry (:doc "|internal function for appending all elements of a list onto a mutable buffer list\\nSyntax: (&buf-list:concat buf xs)\\nParams: buf (buf-list), xs (list)\\nReturns: buf-list\\nMutates buf by appending all elements from xs; returns the same buf")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ let
                buf $ &buf-list:new
              &buf-list:concat buf $ [] 1 2 3
              assert= 3 $ &buf-list:count buf
          :schema $ :: 'Fn
            {} (:return 'Tag)
              :args $ [] 'Tag (:: 'List 'T)
              :generics $ [] 'T
          :tags $ #{} :builtin :internal :state
        |&buf-list:count $ %{} :CodeEntry (:doc "|internal function for getting the element count of a mutable buffer list\\nSyntax: (&buf-list:count buf)\\nParams: buf (buf-list)\\nReturns: number\\nReturns the number of elements currently in the buffer")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ let
                buf $ &buf-list:new
              &buf-list:push buf 42
              assert= 1 $ &buf-list:count buf
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Tag
          :tags $ #{} :builtin :internal :state
        |&buf-list:new $ %{} :CodeEntry (:doc "|internal function for creating a new mutable buffer list\\nSyntax: (&buf-list:new)\\nParams: none\\nReturns: buf-list\\nCreates a new empty mutable append-only buffer list, used for efficient incremental accumulation before converting to an immutable list")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ assert= 0
              &buf-list:count $ &buf-list:new
          :schema $ :: 'Fn
            {} (:return 'Tag)
              :args $ []
          :tags $ #{} :builtin :internal :state
        |&buf-list:push $ %{} :CodeEntry (:doc "|internal function for pushing an item onto a mutable buffer list\\nSyntax: (&buf-list:push buf item)\\nParams: buf (buf-list), item (any)\\nReturns: buf-list\\nMutates buf by appending item to end; returns the same buf")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ let
                buf $ &buf-list:new
              &buf-list:push buf 10
              &buf-list:push buf 20
              assert= 2 $ &buf-list:count buf
          :schema $ :: 'Fn
            {} (:return 'Tag)
              :args $ [] 'Tag 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal :state
        |&buf-list:to-list $ %{} :CodeEntry (:doc "|internal function for converting a mutable buffer list to an immutable list\\nSyntax: (&buf-list:to-list buf)\\nParams: buf (buf-list)\\nReturns: list\\nFreezes the mutable buffer into a regular immutable list")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ let
                buf $ &buf-list:new
              &buf-list:push buf 1
              &buf-list:push buf 2
              assert= ([] 1 2) (&buf-list:to-list buf)
          :schema $ :: 'Fn
            {}
              :args $ [] 'Tag 'T
              :generics $ [] 'T
              :return $ :: 'List 'T
          :tags $ #{} :builtin :internal :state
        |&buffer $ %{} :CodeEntry (:doc "|internal function for buffer operations\nSyntax: (&buffer data)\nParams: data (list of numbers or bytes)\nReturns: buffer object\nCreates a binary buffer from list of byte values")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Buffer)
              :args $ [] 'Dynamic
          :tags $ #{} :builtin :internal :state
        |&call-spread $ %{} :CodeEntry (:doc "|internal syntax for handling & spreading in function calls\nSyntax: (&call-spread fn args)\nParams: fn (function), args (list with spread)\nReturns: function call result\nHandles argument spreading in function calls")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal :syntax
        |&case $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro &case (item default pattern & others)
              if
                not $ and (list? pattern)
                  &= 2 $ &list:count pattern
                raise $ str-spaced "|`case` expects pattern in a pair, got:" pattern
              let
                  x $ &list:first pattern
                  branch $ last pattern
                quasiquote $ if (&= ~item ~x) ~branch
                  ~ $ if (&list:empty? others) default
                    quasiquote $ &case ~item ~default ~@others
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic 'Dynamic 'Dynamic)
          :tags $ #{} :internal :macro
        |&cirru-nth $ %{} :CodeEntry (:doc "|internal function for Cirru nth operation\nSyntax: (&cirru-nth cirru-list index)\nParams: cirru-list (cirru quote list), index (number)\nReturns: cirru node\nGets nth element from Cirru list node, errors if index is out of bounds")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'CirruQuote)
              :args $ [] 'CirruQuote 'Number
          :tags $ #{} :builtin :internal
        |&cirru-quote:to-list $ %{} :CodeEntry (:doc "|internal function for converting Cirru quote to list\nSyntax: (&cirru-quote:to-list quote)\nParams: quote (cirru-quote)\nReturns: list\nConverts Cirru quote structure to regular list")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'List)
              :args $ [] 'CirruQuote
          :tags $ #{} :builtin :internal
        |&cirru-type $ %{} :CodeEntry (:doc "|internal function for getting Cirru type\nSyntax: (&cirru-type cirru-node)\nParams: cirru-node (cirru quote)\nReturns: keyword (:leaf or :list)\nReturns type of Cirru node, either :leaf for atoms or :list for expressions")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Tag)
              :args $ [] 'CirruQuote
          :tags $ #{} :builtin :internal
        |&compare $ %{} :CodeEntry (:doc "|internal function for native comparison\nSyntax: (&compare a b)\nParams: a (any), b (any)\nReturns: number (-1, 0, or 1)\nPerforms three-way comparison returning -1 (less), 0 (equal), or 1 (greater)")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Dynamic 'Dynamic
          :tags $ #{} :builtin :internal
        |&core-fn-impls $ %{} :CodeEntry (:doc "|Built-in implementation list for fn")
          :code $ quote
            def &core-fn-impls $ [] &core-fn-methods (&impl::new Show internal/&core-show-impl)
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :internal
        |&core-fn-methods $ %{} :CodeEntry (:doc |)
          :code $ quote
            def &core-fn-methods $ &impl::new :&core-fn-methods
              :: :call $ defn &fn:call (f & args) (f & args)
              :: :call-args $ defn &fn:call-args (f args) (f & args)
              :: :map &fn:map
              :: :bind &fn:bind
              :: :mappend &fn:mappend
              :: :apply &fn:apply
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :internal
        |&core-list-impls $ %{} :CodeEntry (:doc "|Built-in implementation list for list\nNOTE: ordering matters; &core-list-methods must come before internal/&core-add-list-impl, otherwise list .add may be shadowed by Add trait :add.")
          :code $ quote
            def &core-list-impls $ [] &core-list-methods (&impl::new Show internal/&core-show-impl) (&impl::new Eq internal/&core-eq-impl) (&impl::new Add internal/&core-add-list-impl) (&impl::new Len internal/&core-len-list-impl) (&impl::new Mappable internal/&core-mappable-list-impl) (&impl::new Countable internal/&core-countable-list-impl) (&impl::new Contains internal/&core-contains-list-impl)
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :internal
        |&core-list-methods $ %{} :CodeEntry (:doc |)
          :code $ quote
            def &core-list-methods $ &impl::new :&core-list-methods (:: :any? any?) (:: :add append) (:: :append append) (:: :assoc &list:assoc) (:: :assoc-after &list:assoc-after) (:: :assoc-before &list:assoc-before) (:: :bind mapcat) (:: :butlast butlast) (:: :concat &list:concat) (:: :contains? &list:contains?) (:: :includes? &list:includes?) (:: :count &list:count) (:: :drop drop) (:: :each each) (:: :empty &list:empty) (:: :empty? &list:empty?) (:: :filter &list:filter) (:: :filter-not filter-not) (:: :find find) (:: :find-index find-index) (:: :find-last &list:find-last) (:: :find-last-index &list:find-last-index) (:: :foldl foldl) (:: :get &list:nth) (:: :get-in get-in) (:: :group-by group-by) (:: :index-of index-of) (:: :join join) (:: :join-str join-str) (:: :last-index-of &list:last-index-of) (:: :map &list:map) (:: :map-indexed map-indexed) (:: :mappend &list:mappend) (:: :max &list:max) (:: :min &list:min) (:: :nth &list:nth) (:: :pairs-map pairs-map) (:: :prepend prepend) (:: :reduce reduce) (:: :reverse &list:reverse) (:: :slice &list:slice) (:: :sort sort) (:: :sort-by &list:sort-by) (:: :take take) (:: :take-last take-last) (:: :to-set &list:to-set) (:: :first &list:first) (:: :rest &list:rest) (:: :dissoc &list:dissoc) (:: :to-list identity) (:: :map-pair &list:map-pair) (:: :filter-pair &list:filter-pair) (:: :apply &list:apply) (:: :flatten &list:flatten)
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :internal
        |&core-map-impls $ %{} :CodeEntry (:doc "|Built-in implementation list for map")
          :code $ quote
            def &core-map-impls $ [] &core-map-methods (&impl::new Show internal/&core-show-impl) (&impl::new Eq internal/&core-eq-impl) (&impl::new Len internal/&core-len-map-impl) (&impl::new Mappable internal/&core-mappable-map-impl) (&impl::new Countable internal/&core-countable-map-impl) (&impl::new Contains internal/&core-contains-map-impl)
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :internal
        |&core-map-methods $ %{} :CodeEntry (:doc |)
          :code $ quote
            def &core-map-methods $ &impl::new :&core-map-methods (:: :add &map:add-entry) (:: :assoc &map:assoc) (:: :common-keys &map:common-keys) (:: :contains? &map:contains?) (:: :count &map:count) (:: :destruct &map:destruct) (:: :diff-keys &map:diff-keys) (:: :diff-new &map:diff-new) (:: :diff-triple &map:diff-triple) (:: :dissoc &map:dissoc) (:: :empty &map:empty) (:: :empty? &map:empty?) (:: :filter &map:filter) (:: :filter-kv &map:filter-kv) (:: :get &map:get) (:: :get-in get-in) (:: :includes? &map:includes?) (:: :keys keys) (:: :map &map:map) (:: :map-kv map-kv) (:: :map-list &map:map-list) (:: :mappend merge) (:: :merge merge) (:: :to-list &map:to-list) (:: :to-map identity) (:: :to-pairs to-pairs) (:: :values vals)
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :internal
        |&core-number-impls $ %{} :CodeEntry (:doc "|Built-in implementation list for number")
          :code $ quote
            def &core-number-impls $ [] &core-number-methods (&impl::new Show internal/&core-show-impl) (&impl::new Eq internal/&core-eq-impl) (&impl::new Add internal/&core-add-number-impl) (&impl::new Multiply internal/&core-multiply-number-impl) (&impl::new Compare internal/&core-compare-number-impl)
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :internal
        |&core-number-methods $ %{} :CodeEntry (:doc |)
          :code $ quote
            def &core-number-methods $ &impl::new :&core-number-methods (:: :ceil ceil) (:: :empty &number:empty) (:: :floor floor) (:: :format &number:format) (:: :display-by &number:display-by) (:: :inc inc) (:: :pow pow) (:: :round round) (:: :round? round?) (:: :fract &number:fract) (:: :sqrt sqrt) (:: :negate negate) (:: :rem &number:rem) (:: :compare &compare)
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :internal
        |&core-record-impls $ %{} :CodeEntry (:doc "|Built-in implementation list for record")
          :code $ quote
            def &core-record-impls $ [] &core-record-methods (&impl::new Show internal/&core-show-impl) (&impl::new Eq internal/&core-eq-impl) (&impl::new Countable internal/&core-countable-record-impl) (&impl::new Contains internal/&core-contains-record-impl)
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :internal
        |&core-record-methods $ %{} :CodeEntry (:doc |)
          :code $ quote
            def &core-record-methods $ &impl::new :&core-record-methods (:: :count &record:count) (:: :contains? &record:contains?) (:: :get &record:get) (:: :nth &record:nth) (:: :assoc &record:assoc) (:: :to-map &record:to-map)
              :: :empty? $ defn &record:empty?-impl (x)
                &= 0 $ &record:count x
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :internal
        |&core-scalar-impls $ %{} :CodeEntry (:doc "|Built-in nominal Show/Eq implementation list for scalar literals")
          :code $ quote
            def &core-scalar-impls $ [] (&impl::new Show internal/&core-show-impl) (&impl::new Eq internal/&core-eq-impl)
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :internal
        |&core-set-impls $ %{} :CodeEntry (:doc "|Built-in implementation list for set")
          :code $ quote
            def &core-set-impls $ [] &core-set-methods (&impl::new Show internal/&core-show-impl) (&impl::new Eq internal/&core-eq-impl) (&impl::new Len internal/&core-len-set-impl) (&impl::new Mappable internal/&core-mappable-set-impl) (&impl::new Countable internal/&core-countable-set-impl) (&impl::new Contains internal/&core-contains-set-impl)
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :internal
        |&core-set-methods $ %{} :CodeEntry (:doc |)
          :code $ quote
            def &core-set-methods $ &impl::new :&core-set-methods (:: :add include) (:: :contains? &set:includes?) (:: :count &set:count) (:: :destruct &set:destruct) (:: :difference difference) (:: :empty &set:empty) (:: :empty? &set:empty?) (:: :exclude exclude) (:: :filter &set:filter) (:: :include include) (:: :includes? &set:includes?) (:: :intersection intersection) (:: :map &set:map) (:: :mappend union) (:: :max &set:max) (:: :min &set:min) (:: :to-list &set:to-list) (:: :to-set identity) (:: :union union)
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :internal
        |&core-string-impls $ %{} :CodeEntry (:doc "|Built-in implementation list for string")
          :code $ quote
            def &core-string-impls $ [] &core-string-methods (&impl::new Show internal/&core-show-impl) (&impl::new Eq internal/&core-eq-impl) (&impl::new Add internal/&core-add-string-impl) (&impl::new Len internal/&core-len-string-impl) (&impl::new Countable internal/&core-countable-string-impl) (&impl::new Contains internal/&core-contains-string-impl) (&impl::new Compare internal/&core-compare-string-impl)
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :internal
        |&core-string-methods $ %{} :CodeEntry (:doc |)
          :code $ quote
            def &core-string-methods $ &impl::new :&core-string-methods (:: :blank? blank?) (:: :count &str:count) (:: :empty &str:empty) (:: :ends-with? ends-with?) (:: :get &str:nth) (:: :parse-float parse-float) (:: :replace &str:replace) (:: :split split) (:: :split-lines split-lines) (:: :starts-with? starts-with?) (:: :strip-prefix strip-prefix) (:: :strip-suffix strip-suffix) (:: :slice &str:slice) (:: :trim trim) (:: :empty? &str:empty?) (:: :contains? &str:contains?) (:: :includes? &str:includes?) (:: :nth &str:nth) (:: :first &str:first) (:: :rest &str:rest) (:: :pad-left &str:pad-left) (:: :pad-right &str:pad-right) (:: :find-index &str:find-index) (:: :get-char-code get-char-code) (:: :escape &str:escape) (:: :mappend &str:concat) (:: :compare &str:compare)
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :internal
        |&core-tuple-impls $ %{} :CodeEntry (:doc "|Built-in implementation list for tuple")
          :code $ quote
            def &core-tuple-impls $ [] &core-tuple-methods (&impl::new Show internal/&core-show-impl) (&impl::new Eq internal/&core-eq-impl) (&impl::new Countable internal/&core-countable-tuple-impl) (&impl::new Contains internal/&core-contains-tuple-impl)
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :internal
        |&core-tuple-methods $ %{} :CodeEntry (:doc |)
          :code $ quote
            def &core-tuple-methods $ &impl::new :&core-tuple-methods (:: :count &tuple:count) (:: :nth &tuple:nth) (:: :get &tuple:nth) (:: :assoc &tuple:assoc)
              :: :first $ defn &tuple:first-impl (x) (&tuple:nth x 0)
              :: :empty? $ defn &tuple:empty?-impl (x)
                &= 0 $ &tuple:count x
              :: :contains? $ defn &tuple:contains?-impl (x k)
                if (&>= k 0)
                  &< k $ &tuple:count x
                  , false
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :internal
        |&data-to-code $ %{} :CodeEntry (:doc "|internal function for converting data to code\nSyntax: (&data-to-code data)\nParams: data (EDN data)\nReturns: quoted code\nConverts EDN data structure back to executable code")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal :meta
        |&difference $ %{} :CodeEntry (:doc "|internal function for set difference\nSyntax: (&difference set1 set2)\nParams: set1 (set), set2 (set)\nReturns: set\nReturns elements in set1 but not in set2")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Set 'T) (:: 'Set 'T)
              :generics $ [] 'T
              :return $ :: 'Set 'T
          :tags $ #{} :builtin :internal
        |&display-stack $ %{} :CodeEntry (:doc "|internal function for displaying call stack\nSyntax: (&display-stack)\nParams: none\nReturns: string representation of call stack\nReturns formatted string showing current call stack for debugging")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ []
          :tags $ #{} :builtin :internal :io :log
        |&doseq $ %{} :CodeEntry (:doc "|Side-effect traversal macro. Iterates over a binding pair, executing the body for each element and returning nil.")
          :code $ quote
            defmacro &doseq (pair & body)
              if
                not $ and (list? pair)
                  &= 2 $ &list:count pair
                raise $ str-spaced "|doseq expects a pair, got:" pair
              let
                  name $ &list:first pair
                  xs0 $ last pair
                quasiquote $ foldl ~xs0 nil
                  defn doseq-fn% (_acc ~name) ~@body
          :examples $ []
            quote $ do
              defatom *seen $ []
              &doseq
                n $ [] 1 2
                reset! *seen $ append (deref *seen) n
              assert= ([] 1 2) (deref *seen)
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :effect :internal :macro
        |&enum::new $ %{} :CodeEntry (:doc "|internal function for creating enum definitions\nSyntax: (&enum::new name (variant type...) ...)\nParams: name (tag), variant entries (list)\nReturns: enum prototype value\nCreates enum variants and payload type annotations")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ &enum::new :Result ([] :ok :number) ([] :err :string)
          :schema $ :: 'Fn
            {} (:return 'Enum)
              :args $ [] 'Tag
          :tags $ #{} :builtin :internal
        |&enum:impl-traits $ %{} :CodeEntry (:doc "|internal function for enum trait impl attachment\nSyntax: (&enum:impl-traits enum impls)\nParams: enum (enum), impls (record)\nReturns: enum value with trait implementations\nAttaches impls info to an enum prototype")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Enum)
              :args $ [] 'Enum
          :tags $ #{} :builtin :internal
        |&exclude $ %{} :CodeEntry (:doc "|internal function for excluding from set\nSyntax: (&exclude set element)\nParams: set (set), element (any)\nReturns: set\nReturns new set with element excluded")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Set 'T) 'T
              :generics $ [] 'T
              :return $ :: 'Set 'T
          :tags $ #{} :builtin :internal
        |&extract-code-into-edn $ %{} :CodeEntry (:doc "|internal function for extracting code into EDN\nSyntax: (&extract-code-into-edn code)\nParams: code (quoted code)\nReturns: EDN data structure\nExtracts code structure into EDN format for serialization")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal :meta
        |&fn:apply $ %{} :CodeEntry (:doc "|internal helper for fn :apply method entry")
          :code $ quote
            defn &fn:apply (f g)
              fn (x)
                g x $ f x
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Fn)
              :args $ []
                :: 'Fn $ {} (:return 'B)
                  :args $ [] 'A
                :: 'Fn $ {} (:return 'C)
                  :args $ [] 'A 'B
              :generics $ [] 'A 'B 'C
          :tags $ #{} :internal
        |&fn:bind $ %{} :CodeEntry (:doc "|internal helper for fn :bind method entry")
          :code $ quote
            defn &fn:bind (m f)
              fn (x)
                f (m x) x
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Fn)
              :args $ []
                :: 'Fn $ {} (:return 'B)
                  :args $ [] 'A
                :: 'Fn $ {} (:return 'C)
                  :args $ [] 'B 'A
              :generics $ [] 'A 'B 'C
          :tags $ #{} :internal
        |&fn:map $ %{} :CodeEntry (:doc "|internal helper for fn :map method entry")
          :code $ quote
            defn &fn:map (f g)
              fn (x)
                f $ g x
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Fn)
              :args $ []
                :: 'Fn $ {} (:return 'C)
                  :args $ [] 'B
                :: 'Fn $ {} (:return 'B)
                  :args $ [] 'A
              :generics $ [] 'A 'B 'C
          :tags $ #{} :internal
        |&fn:mappend $ %{} :CodeEntry (:doc "|internal helper for fn :mappend method entry")
          :code $ quote
            defn &fn:mappend (f g)
              fn (x)
                &let
                  v1 $ f x
                  &let
                    v2 $ g x
                    if (list? v1) (&list:concat v1 v2)
                      if (map? v1) (merge v1 v2)
                        if (set? v1) (union v1 v2)
                          if (string? v1) (&str:concat v1 v2) (.mappend v1 v2)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Fn)
              :args $ []
                :: 'Fn $ {} (:return 'B)
                  :args $ [] 'A
                :: 'Fn $ {} (:return 'B)
                  :args $ [] 'A
              :generics $ [] 'A 'B
          :tags $ #{} :internal
        |&format-ternary-tree $ %{} :CodeEntry (:doc "|internal function for formatting ternary tree\nSyntax: (&format-ternary-tree tree)\nParams: tree (ternary tree structure)\nReturns: formatted string\nFormats internal ternary tree data structure for debugging")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'List
          :tags $ #{} :builtin :internal
        |&get-calcit-backend $ %{} :CodeEntry (:doc "|internal function for getting Calcit backend\nSyntax: (&get-calcit-backend)\nParams: none\nReturns: keyword indicating backend\nReturns current backend like :cr (Calcit Runner) or :js (JavaScript)")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Tag)
              :args $ []
          :tags $ #{} :builtin :internal :io
        |&get-calcit-running-mode $ %{} :CodeEntry (:doc "|internal function for getting Calcit running mode\nSyntax: (&get-calcit-running-mode)\nParams: none\nReturns: keyword indicating mode\nReturns current running mode like :dev, :release, or :test")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Tag)
              :args $ []
          :tags $ #{} :builtin :internal :io
        |&get-def-doc $ %{} :CodeEntry (:doc "|internal function for reading definition doc metadata (cr/eval runtime only)\nSyntax: (&get-def-doc ns/def)\nParams: target (string or symbol in `ns/def` form)\nReturns: string\nReturns the `:doc` text stored on a loaded definition, or empty string when missing. Not available in calcit-js.")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ assert= true
              includes? (&get-def-doc |calcit.core/map) |map
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'Dynamic
          :tags $ #{} :builtin :internal :io :meta
        |&get-def-schema $ %{} :CodeEntry (:doc "|internal function for reading definition schema metadata (cr/eval runtime only)\nSyntax: (&get-def-schema ns/def)\nParams: target (string or symbol in `ns/def` form)\nReturns: EDN data\nReturns the `:schema` value stored on a loaded definition as EDN data. Not available in calcit-js.")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ assert= :fn
              &tuple:nth (&get-def-schema |calcit.core/map) 0
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic
          :tags $ #{} :builtin :internal :io :meta
        |&get-os $ %{} :CodeEntry (:doc "|internal function for getting OS information\nSyntax: (&get-os)\nParams: none\nReturns: keyword indicating OS\nReturns current operating system like :linux, :macos, :windows")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Tag)
              :args $ []
          :tags $ #{} :builtin :internal :io
        |&hash $ %{} :CodeEntry (:doc "|internal function for hashing\nSyntax: (&hash value)\nParams: value (any)\nReturns: number (hash code)\nComputes hash code for any Calcit value for use in hash tables")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Dynamic
          :tags $ #{} :builtin :internal
        |&impl::new $ %{} :CodeEntry (:doc "|internal function for creating trait impl records\nSyntax: (&impl::new trait-or-name (method value) ...)\nParams: trait-or-name (trait/tag/symbol/string), method entries (pairs)\nReturns: impl\nAccepts method key as .method, :tag, symbol, or string")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ do
              deftrait DemoTrait $ .show :fn
              &impl::new DemoTrait $ :: .show
                fn (x) x
            quote $ &impl::new :DemoImpl
              [] :show $ fn (x) x
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal
        |&impl:get $ %{} :CodeEntry (:doc "|internal function for getting impl entry by name\nSyntax: (&impl:get impl name)\nParams: impl (impl), name (tag/string/symbol/.method)\nReturns: any\nReturns impl entry value by method name")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ do
              deftrait DemoTrait $ .show :fn
              def DemoImpl $ &impl::new DemoTrait
                :: .show $ fn (x) x
              fn? $ &impl:get DemoImpl .show
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Impl 'Dynamic
          :tags $ #{} :builtin :internal
        |&impl:nth $ %{} :CodeEntry (:doc "|internal function for getting impl entry by index\nSyntax: (&impl:nth impl index)\nParams: impl (impl), index (number)\nReturns: any\nReturns impl entry value by index")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Impl 'Number
          :tags $ #{} :builtin :internal
        |&include $ %{} :CodeEntry (:doc "|internal function for including in set\nSyntax: (&include set element)\nParams: set (set), element (any)\nReturns: set\nReturns new set with element included")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Set 'T) 'T
              :generics $ [] 'T
              :return $ :: 'Set 'T
          :tags $ #{} :builtin :internal
        |&init-builtin-impls! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &init-builtin-impls! () (; "this function to make sure builtin impls are loaded") (identity &core-number-impls) (identity &core-string-impls) (identity &core-set-impls) (identity &core-list-impls) (identity &core-map-impls) (identity &core-fn-impls) (identity &core-tuple-impls) (identity &core-record-impls) (identity &core-scalar-impls) (identity Add) (identity Eq) (identity Len) (identity Mappable) (identity Multiply) (identity Show)
              if
                &= (&get-calcit-backend) :js
                register-calcit-builtin-impls $ &js-object :number &core-number-impls :string &core-string-impls :set &core-set-impls :list &core-list-impls :map &core-map-impls :fn &core-fn-impls :tuple &core-tuple-impls :record &core-record-impls :scalar &core-scalar-impls
                ;nil
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
          :tags $ #{} :internal
        |&let $ %{} :CodeEntry (:doc "|internal syntax for local binding (binds only 1 local)\nSyntax: (&let [binding value] body)\nParams: binding (symbol), value (any), body (expression)\nReturns: result of body with binding in scope\nCreates a local binding for a single variable")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ assert= 6
              &let
                x $ + 1 2
                * x 2
            quote $ assert= |done
              &let (label |done) label
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal :syntax
        |&list-match-internal $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro &list-match-internal (v branch1 pair branch2)
              quasiquote $ if (&list:empty? ~v)
                &let () ~@branch1
                &let
                    ~ $ first pair
                    &list:nth ~v 0
                  &let
                      ~ $ &list:nth pair 1
                      &list:slice ~v 1
                    &let () ~@branch2
          :examples $ []
          :schema $ :: 'Macro
            {}
              :args $ [] (:: 'List 'T) 'Dynamic 'Dynamic 'Dynamic
              :generics $ [] 'T
          :tags $ #{} :internal :macro
        |&list:append $ %{} :CodeEntry (:doc |)
          :code $ quote (&runtime-implementation)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T) 'T
              :generics $ [] 'T
              :return $ :: 'List 'T
          :tags $ #{} :builtin :internal
        |&list:apply $ %{} :CodeEntry (:doc "|internal helper for list :apply method entry")
          :code $ quote
            defn &list:apply (xs fs)
              &list:concat & $ map fs
                fn (f)
                  map xs $ fn (x) (f x)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T) (:: 'List 'Fn)
              :generics $ [] 'T 'U
              :return $ :: 'List 'U
          :tags $ #{} :internal
        |&list:assoc $ %{} :CodeEntry (:doc "|internal function for list association\nSyntax: (&list:assoc list index element)\nParams: list (list), index (number), element (any)\nReturns: list\nReturns new list with element at specified index")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T) 'Number 'T
              :generics $ [] 'T
              :return $ :: 'List 'T
          :tags $ #{} :builtin :internal
        |&list:assoc-after $ %{} :CodeEntry (:doc "|internal function for associating after element\nSyntax: (&list:assoc-after list target element)\nParams: list (list), target (any), element (any)\nReturns: list\nInserts element after first occurrence of target")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T) 'Number 'T
              :generics $ [] 'T
              :return $ :: 'List 'T
          :tags $ #{} :builtin :internal
        |&list:assoc-before $ %{} :CodeEntry (:doc "|internal function for associating before element\nSyntax: (&list:assoc-before list target element)\nParams: list (list), target (any), element (any)\nReturns: list\nInserts element before first occurrence of target")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T) 'Number 'T
              :generics $ [] 'T
              :return $ :: 'List 'T
          :tags $ #{} :builtin :internal
        |&list:butlast $ %{} :CodeEntry (:doc |)
          :code $ quote (&runtime-implementation)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T)
              :generics $ [] 'T
              :return $ :: 'List 'T
          :tags $ #{} :builtin :internal
        |&list:concat $ %{} :CodeEntry (:doc "|internal function for concatenating lists\nSyntax: (&list:concat list1 list2)\nParams: list1 (list), list2 (list)\nReturns: list\nReturns new list with elements from both lists")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ []
              :generics $ [] 'T
              :rest $ :: 'List 'T
              :return $ :: 'List 'T
          :tags $ #{} :builtin :internal
        |&list:contains? $ %{} :CodeEntry (:doc "|internal function for checking if list contains element\nSyntax: (&list:contains? list element)\nParams: list (list), element (any)\nReturns: boolean\nReturns true if list contains element")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] (:: 'List 'T) 'Number
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |&list:count $ %{} :CodeEntry (:doc "|internal function for counting list elements\nSyntax: (&list:count list)\nParams: list (list)\nReturns: number\nReturns number of elements in list")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] (:: 'List 'T)
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |&list:dissoc $ %{} :CodeEntry (:doc "|internal function for list dissociation\nSyntax: (&list:dissoc list index)\nParams: list (list), index (number)\nReturns: list\nReturns new list without element at specified index")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T) 'Number
              :generics $ [] 'T
              :return $ :: 'List 'T
          :tags $ #{} :builtin :internal
        |&list:distinct $ %{} :CodeEntry (:doc "|internal function for getting distinct list elements\nSyntax: (&list:distinct list)\nParams: list (list)\nReturns: list\nReturns new list with duplicate elements removed")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T)
              :generics $ [] 'T
              :return $ :: 'List 'T
          :tags $ #{} :builtin :internal
        |&list:empty $ %{} :CodeEntry (:doc "|internal helper for list :empty method entry")
          :code $ quote
            defn &list:empty (_xs) ([])
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T)
              :generics $ [] 'T
              :return $ :: 'List 'T
          :tags $ #{} :internal
        |&list:empty? $ %{} :CodeEntry (:doc "|internal function for checking if list is empty\nSyntax: (&list:empty? list)\nParams: list (list)\nReturns: boolean\nReturns true if list has no elements")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] (:: 'List 'T)
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |&list:filter $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:filter (xs f)
              reduce xs ([])
                defn %&list:filter (acc x)
                  if (f x) (append acc x) acc
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T)
                :: 'Fn $ {} (:return 'Bool)
                  :args $ [] 'T
              :generics $ [] 'T
              :return $ :: 'List 'T
          :tags $ #{} :internal
        |&list:filter-pair $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:filter-pair (xs f)
              if (list? xs)
                &list:filter xs $ defn %filter-pair (pair)
                  assert "|expected a pair" $ and (list? pair)
                    = 2 $ count pair
                  f (nth pair 0) (nth pair 1)
                raise $ str-spaced "|expected list or map from `filter-pair`, got:" xs
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'P) 'Fn
              :generics $ [] 'P
              :return $ :: 'List 'P
          :tags $ #{} :internal
        |&list:find-last $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:find-last (xs f)
              foldr-shortcut xs (;nil) (;nil)
                fn (_acc x)
                  if (f x) (:: true x)
                    :: false $ ;nil
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T)
                :: 'Fn $ {} (:return 'Bool)
                  :args $ [] 'T
              :generics $ [] 'T
              :return $ :: 'Optional 'T
          :tags $ #{} :internal
        |&list:find-last-index $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:find-last-index (xs f)
              foldr-shortcut xs
                dec $ count xs
                ;nil
                fn (idx x)
                  if (f x) (:: true idx)
                    :: false $ &- 1 idx
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T)
                :: 'Fn $ {} (:return 'Bool)
                  :args $ [] 'T
              :generics $ [] 'T
              :return $ :: 'Optional 'Number
          :tags $ #{} :internal
        |&list:first $ %{} :CodeEntry (:doc "|internal function for getting first list element\nSyntax: (&list:first list)\nParams: list (list)\nReturns: any or nil\nReturns first element of list, nil if empty")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T)
              :generics $ [] 'T
              :return $ :: 'Optional 'T
          :tags $ #{} :builtin :internal
        |&list:flatten $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:flatten (xs)
              if (list? xs)
                &list:concat & $ map xs &list:flatten
                [] xs
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'List)
              :args $ [] 'Dynamic
          :tags $ #{} :internal
        |&list:foldl $ %{} :CodeEntry (:doc |)
          :code $ quote (&runtime-implementation)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'U)
              :args $ [] (:: 'List 'T) 'U
                :: 'Fn $ {} (:return 'U)
                  :args $ [] 'U 'T
              :generics $ [] 'T 'U
          :tags $ #{} :builtin :internal
        |&list:foldl-shortcut $ %{} :CodeEntry (:doc |)
          :code $ quote (&runtime-implementation)
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal
        |&list:includes? $ %{} :CodeEntry (:doc "|internal function for checking if list includes element\nSyntax: (&list:includes? list element)\nParams: list (list), element (any)\nReturns: boolean\nReturns true if list includes element (alias for contains?)")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] (:: 'List 'T) 'T
              :generics $ [] 'T
          :tags $ #{} :alias :builtin :internal
        |&list:last $ %{} :CodeEntry (:doc |)
          :code $ quote (&runtime-implementation)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T)
              :generics $ [] 'T
              :return $ :: 'Optional 'T
          :tags $ #{} :builtin :internal
        |&list:last-index-of $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:last-index-of (xs item)
              foldr-shortcut xs
                dec $ count xs
                ;nil
                fn (idx x)
                  if (&= item x) (:: true idx)
                    :: false $ &- 1 idx
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T) 'T
              :generics $ [] 'T
              :return $ :: 'Optional 'Number
          :tags $ #{} :internal
        |&list:map $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:map (xs f)
              foldl xs ([])
                defn %&list:map (acc x)
                  append acc $ f x
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T)
                :: 'Fn $ {} (:return 'U)
                  :args $ [] 'T
              :generics $ [] 'T 'U
              :return $ :: 'List 'U
          :tags $ #{} :internal
        |&list:map-pair $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:map-pair (xs f)
              if (list? xs)
                map xs $ defn %map-pair (pair)
                  assert "|expected a pair" $ and (list? pair)
                    = 2 $ count pair
                  f (nth pair 0) (nth pair 1)
                raise $ str-spaced "|expected list or map from `map-pair`, got:" xs
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'P) 'Fn
              :generics $ [] 'P 'U
              :return $ :: 'List 'U
          :tags $ #{} :internal
        |&list:mappend $ %{} :CodeEntry (:doc "|internal helper for list :mappend method entry")
          :code $ quote
            defn &list:mappend (x y) (&list:concat x y)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T) (:: 'List 'T)
              :generics $ [] 'T
              :return $ :: 'List 'T
          :tags $ #{} :internal
        |&list:max $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:max (xs)
              if (&list:empty? xs) (&list:first xs)
                &list:max-loop (&list:rest xs) (&list:first xs)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'Number)
              :return $ :: 'Optional 'Number
          :tags $ #{} :internal
        |&list:max-loop $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:max-loop (xs acc)
              if (&list:empty? xs) acc $ &let
                x $ &list:first xs
                recur (&list:rest xs)
                  if (&> x acc) x acc
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] (:: 'List 'Number) 'Number
          :tags $ #{} :internal
        |&list:min $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:min (xs)
              if (&list:empty? xs) (&list:first xs)
                &list:min-loop (&list:rest xs) (&list:first xs)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'Number)
              :return $ :: 'Optional 'Number
          :tags $ #{} :internal
        |&list:min-loop $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:min-loop (xs acc)
              if (&list:empty? xs) acc $ &let
                x $ &list:first xs
                recur (&list:rest xs)
                  if (&< x acc) x acc
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] (:: 'List 'Number) 'Number
          :tags $ #{} :internal
        |&list:nth $ %{} :CodeEntry (:doc "|internal function for getting nth list element\nSyntax: (&list:nth list index)\nParams: list (list), index (number)\nReturns: any or nil\nReturns element at index, nil if index out of bounds")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Tag)
              :args $ [] (:: 'List 'T) 'Number
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |&list:prepend $ %{} :CodeEntry (:doc |)
          :code $ quote (&runtime-implementation)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'T (:: 'List 'T)
              :generics $ [] 'T
              :return $ :: 'List 'T
          :tags $ #{} :builtin :internal
        |&list:range $ %{} :CodeEntry (:doc |)
          :code $ quote (&runtime-implementation)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'Number 'Number
              :return $ :: 'List 'Number
          :tags $ #{} :builtin :internal
        |&list:rest $ %{} :CodeEntry (:doc "|internal function for getting rest of list\nSyntax: (&list:rest list)\nParams: list (list)\nReturns: list\nReturns list without first element")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T)
              :generics $ [] 'T
              :return $ :: 'List 'T
          :tags $ #{} :builtin :internal
        |&list:reverse $ %{} :CodeEntry (:doc "|internal function for reversing lists\nSyntax: (&list:reverse list)\nParams: list (list)\nReturns: list\nReturns new list with elements in reverse order")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T)
              :generics $ [] 'T
              :return $ :: 'List 'T
          :tags $ #{} :builtin :internal
        |&list:slice $ %{} :CodeEntry (:doc "|internal function for slicing lists\nSyntax: (&list:slice list start end)\nParams: list (list), start (number), end (number)\nReturns: list\nReturns sublist from start to end index")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T) 'Number 'Number
              :generics $ [] 'T
              :return $ :: 'List 'T
          :tags $ #{} :builtin :internal
        |&list:sort $ %{} :CodeEntry (:doc |)
          :code $ quote (&runtime-implementation)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T) (:: 'Optional 'Fn)
              :generics $ [] 'T
              :return $ :: 'List 'T
          :tags $ #{} :builtin :internal
        |&list:sort-by $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:sort-by (xs f)
              if (tag? f)
                sort xs $ defn %&list:sort-by (a b)
                  &compare (get a f) (get b f)
                sort xs $ defn %&list:sort-by (a b)
                  &compare (f a) (f b)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T) 'Dynamic
              :generics $ [] 'T
              :return $ :: 'List 'T
          :tags $ #{} :internal
        |&list:to-set $ %{} :CodeEntry (:doc "|internal function for converting list to set\nSyntax: (&list:to-set list)\nParams: list (list)\nReturns: set\nConverts list to set, removing duplicates")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T)
              :generics $ [] 'T
              :return $ :: 'Set 'T
          :tags $ #{} :builtin :internal
        |&map:add-entry $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &map:add-entry (xs pair)
              assert "|&map:add-entry expected value in a pair" $ and (list? pair)
                &= 2 $ count pair
              &map:assoc xs (nth pair 0) (nth pair 1)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V) (:: 'List 'P)
              :generics $ [] 'K 'V 'P
              :return $ :: 'Map 'K 'V
          :tags $ #{} :internal
        |&map:assoc $ %{} :CodeEntry (:doc "|internal function for map association\nSyntax: (&map:assoc map key value & key-values)\nParams: map (map), key (any), value (any), key-values (any, variadic)\nReturns: map\nReturns new map with key-value associations")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V) 'K 'V
              :generics $ [] 'K 'V
              :return $ :: 'Map 'K 'V
          :tags $ #{} :builtin :internal
        |&map:common-keys $ %{} :CodeEntry (:doc "|internal function for map common keys\nSyntax: (&map:common-keys map1 map2)\nParams: map1 (map), map2 (map)\nReturns: set\nReturns keys common to both maps")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V) (:: 'Map 'K 'W)
              :generics $ [] 'K 'V 'W
              :return $ :: 'Set 'K
          :tags $ #{} :builtin :internal
        |&map:contains? $ %{} :CodeEntry (:doc "|internal function for checking if map contains key\nSyntax: (&map:contains? map key)\nParams: map (map), key (any)\nReturns: boolean\nReturns true if map contains key")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] (:: 'Map 'K 'V) 'K
              :generics $ [] 'K 'V
          :tags $ #{} :builtin :internal
        |&map:count $ %{} :CodeEntry (:doc "|internal function for counting map entries\nSyntax: (&map:count map)\nParams: map (map)\nReturns: number\nReturns number of key-value pairs in map")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] (:: 'Map 'K 'V)
              :generics $ [] 'K 'V
          :tags $ #{} :builtin :internal
        |&map:destruct $ %{} :CodeEntry (:doc "|internal function for map destructuring\nSyntax: (&map:destruct map pattern)\nParams: map (map), pattern (any)\nReturns: map\nDestructs map according to pattern")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V)
              :generics $ [] 'K 'V
              :return $ :: 'Optional 'Tuple
          :tags $ #{} :builtin :internal
        |&map:diff-keys $ %{} :CodeEntry (:doc "|internal function for map diff keys\nSyntax: (&map:diff-keys map1 map2)\nParams: map1 (map), map2 (map)\nReturns: set\nReturns keys that differ between maps")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V) (:: 'Map 'K 'W)
              :generics $ [] 'K 'V 'W
              :return $ :: 'Set 'K
          :tags $ #{} :builtin :internal
        |&map:diff-new $ %{} :CodeEntry (:doc "|internal function for map diff new\nSyntax: (&map:diff-new map1 map2)\nParams: map1 (map), map2 (map)\nReturns: map\nReturns new entries in map2 not in map1")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V) (:: 'Map 'K 'W)
              :generics $ [] 'K 'V 'W
              :return $ :: 'Map 'K 'W
          :tags $ #{} :builtin :internal
        |&map:diff-triple $ %{} :CodeEntry (:doc "|Single-pass map diff returning [drop-keys new-diff common-triples].\nSyntax: (&map:diff-triple a b)\nParams: a (map), b (map)\nReturns: list\nReturns a list of three elements:\n  - drop-keys: set of keys in `a` but not in `b`\n  - new-diff: map of entries in `b` but not in `a`\n  - common-triples: list of [k va vb] for every key present in both maps\n\nMore efficient than calling &map:diff-keys, &map:diff-new, and &map:common-keys separately,\nas it only traverses both maps twice instead of 3+ times.")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ let
                triple $ &map:diff-triple (&{} :a 1 :b 2) (&{} :a 2 :c 3)
              [] (nth triple 0) (nth triple 1)
                count $ nth triple 2
            quote $ let
                triple $ &map:diff-triple (&{} :x 10 :y 20) (&{} :x 10 :y 99 :z 30)
                drop-keys $ nth triple 0
                new-diff $ nth triple 1
                common-triples $ nth triple 2
              list drop-keys new-diff common-triples
          :schema $ :: 'Fn
            {} (:return 'Tuple)
              :args $ [] (:: 'Map 'K 'V) (:: 'Map 'K 'W)
              :generics $ [] 'K 'V 'W
          :tags $ #{} :builtin :internal
        |&map:dissoc $ %{} :CodeEntry (:doc "|internal function for map dissociation\nSyntax: (&map:dissoc map key & keys)\nParams: map (map), key (any), keys (any, variadic)\nReturns: map\nReturns new map without specified keys")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V) 'K
              :generics $ [] 'K 'V
              :return $ :: 'Map 'K 'V
          :tags $ #{} :builtin :internal
        |&map:empty $ %{} :CodeEntry (:doc "|internal helper for producing an empty map value while preserving method signature shape")
          :code $ quote
            defn &map:empty (_xs) (&{})
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V)
              :generics $ [] 'K 'V
              :return $ :: 'Map 'K 'V
          :tags $ #{} :internal
        |&map:empty? $ %{} :CodeEntry (:doc "|internal function for checking if map is empty\nSyntax: (&map:empty? map)\nParams: map (map)\nReturns: boolean\nReturns true if map has no entries")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] (:: 'Map 'K 'V)
              :generics $ [] 'K 'V
          :tags $ #{} :builtin :internal
        |&map:filter $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &map:filter (xs f)
              reduce xs (&{})
                defn %&map:filter (acc x)
                  hint-fn $ {}
                    :args $ [] (:: 'Map 'K 'V) ('P)
                    :return $ :: 'Map 'K 'V
                  if (f x)
                    &map:assoc acc (nth x 0) (nth x 1)
                    , acc
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V)
                :: 'Fn $ {} (:return 'Bool)
                  :args $ [] 'P
              :generics $ [] 'K 'V 'P
              :return $ :: 'Map 'K 'V
          :tags $ #{} :internal
        |&map:filter-kv $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &map:filter-kv (xs f)
              reduce xs (&{})
                defn %map:filter-kv (acc x)
                  hint-fn $ {}
                    :args $ [] (:: 'Map 'K 'V) ('P)
                    :return $ :: 'Map 'K 'V
                  if
                    f (nth x 0) (nth x 1)
                    &map:assoc acc (nth x 0) (nth x 1)
                    , acc
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V)
                :: 'Fn $ {} (:return 'Bool)
                  :args $ [] 'K 'V
              :generics $ [] 'K 'V
              :return $ :: 'Map 'K 'V
          :tags $ #{} :internal
        |&map:get $ %{} :CodeEntry (:doc "|internal function for getting map value\nSyntax: (&map:get map key) or (&map:get map key default)\nParams: map (map), key (any), default (any, optional)\nReturns: any\nGets value for key, returns default if key not found")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V) 'K
              :generics $ [] 'K 'V
              :return $ :: 'Optional 'V
          :tags $ #{} :builtin :internal
        |&map:includes? $ %{} :CodeEntry (:doc "|internal function for checking if map includes key\nSyntax: (&map:includes? map key)\nParams: map (map), key (any)\nReturns: boolean\nReturns true if map includes key (alias for contains?)")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] (:: 'Map 'K 'V) 'K
              :generics $ [] 'K 'V
          :tags $ #{} :alias :builtin :internal
        |&map:keys $ %{} :CodeEntry (:doc |)
          :code $ quote (&runtime-implementation)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V)
              :generics $ [] 'K 'V
              :return $ :: 'Set 'K
          :tags $ #{} :builtin :internal
        |&map:map $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &map:map (xs f)
              foldl xs ({})
                defn &map:map (acc pair)
                  hint-fn $ {}
                    :args $ [] (:: 'Map 'R 'S) ('P)
                    :return $ :: 'Map 'R 'S
                  &let
                    result $ f pair
                    assert "|expected pair returned when mapping hashmap" $ and (list? result)
                      &= 2 $ &list:count result
                    &map:assoc acc (nth result 0) (nth result 1)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V)
                :: 'Fn $ {} (:return 'Q)
                  :args $ [] 'P
              :generics $ [] 'K 'V 'P 'Q 'R 'S
              :return $ :: 'Map 'R 'S
          :tags $ #{} :internal
        |&map:map-list $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &map:map-list (xs f)
              if (map? xs)
                foldl xs ([])
                  defn %&map:map-list (acc pair)
                    hint-fn $ {}
                      :args $ [] (:: 'List 'U) ('P)
                      :return $ :: 'List 'U
                    append acc $ f pair
                raise $ str-spaced "|&map:map-list expected a map, got:" xs
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V)
                :: 'Fn $ {} (:return 'U)
                  :args $ [] 'P
              :generics $ [] 'K 'V 'P 'U
              :return $ :: 'List 'U
          :tags $ #{} :internal
        |&map:to-list $ %{} :CodeEntry (:doc "|internal function for converting map to list\nSyntax: (&map:to-list map)\nParams: map (map)\nReturns: list\nConverts map to list of [key value] pairs")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V)
              :generics $ [] 'K 'V
              :return $ :: 'List 'Tuple
          :tags $ #{} :builtin :internal
        |&map:vals $ %{} :CodeEntry (:doc |)
          :code $ quote (&runtime-implementation)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V)
              :generics $ [] 'K 'V
              :return $ :: 'Set 'V
          :tags $ #{} :builtin :internal
        |&max $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &max (a b)
              assert "|expects numbers for &max" $ if (number? a) (number? b)
              if (&> a b) a b
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number 'Number
          :tags $ #{} :internal
        |&merge $ %{} :CodeEntry (:doc "|internal function for merging maps\nSyntax: (&merge map1 map2 & maps)\nParams: map1 (map), map2 (map), maps (map, variadic)\nReturns: map\nMerges multiple maps, later values override earlier ones")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V) (:: 'Map 'K 'V)
              :generics $ [] 'K 'V
              :rest $ :: 'Map 'K 'V
              :return $ :: 'Map 'K 'V
          :tags $ #{} :builtin :internal
        |&merge-non-nil $ %{} :CodeEntry (:doc "|internal function for merging non-nil values\nSyntax: (&merge-non-nil map1 map2 & maps)\nParams: map1 (map), map2 (map), maps (map, variadic)\nReturns: map\nMerges maps, skipping nil values")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V) (:: 'Map 'K 'V)
              :generics $ [] 'K 'V
              :return $ :: 'Map 'K 'V
          :tags $ #{} :builtin :internal
        |&min $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &min (a b)
              assert "|expects numbers for &min" $ if (number? a) (number? b)
              if (&< a b) a b
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number 'Number
          :tags $ #{} :internal
        |&number:display-by $ %{} :CodeEntry (:doc "|internal function for number display by base\nSyntax: (&number:display-by n base)\nParams: n (number), base (integer)\nReturns: string\nDisplays number in specified base (2-36)")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'Number 'Number
          :tags $ #{} :builtin :internal
        |&number:empty $ %{} :CodeEntry (:doc "|internal helper for number :empty method entry")
          :code $ quote
            defn &number:empty (_x) 0
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number
          :tags $ #{} :internal
        |&number:format $ %{} :CodeEntry (:doc "|internal function for number formatting\nSyntax: (&number:format n)\nParams: n (number)\nReturns: string\nFormats number as string representation")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'Number
          :tags $ #{} :builtin :internal
        |&number:fract $ %{} :CodeEntry (:doc "|internal function for number fractional part\nSyntax: (&number:fract n)\nParams: n (number)\nReturns: number\nReturns fractional part of number (n - floor(n))")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number
          :tags $ #{} :builtin :internal
        |&number:rem $ %{} :CodeEntry (:doc "|internal function for number remainder\nSyntax: (&number:rem a b)\nParams: a (number), b (number)\nReturns: number\nReturns remainder of a divided by b")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number 'Number
          :tags $ #{} :builtin :internal
        |&record-match-internal $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro &record-match-internal (value & body)
              if (&list:empty? body)
                quasiquote $ eprintln "|[Warn] record-match found no matched case, missing `_` case?" ~value
                &let
                  pair $ &list:nth body 0
                  if
                    not $ list? pair
                    raise $ str-spaced "|record-match expected arm in list, got:" pair
                  let
                      pattern $ &list:nth pair 0
                    assert "|expected record or symbol as pattern" $ or (record? pattern) (symbol? pattern)
                    if (&= pattern '_)
                      &let ()
                        assert "|record-match expected a branch after `_`" $ &<= 3 (&list:count pair)
                        quasiquote $ &let
                            ~ $ &list:nth pair 1
                            , ~value
                          ~@ $ &list:slice pair 2
                      &let ()
                        assert "|record-match expected an with (proto new-name & body)" $ &<= 3 (&list:count pair)
                        quasiquote $ if (&record:matches? ~value ~pattern)
                          &let
                              ~ $ &list:nth pair 1
                              , ~value
                            ~@ $ &list:slice pair 2
                          &record-match-internal ~value $ ~@ (&list:rest body)
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Record)
          :tags $ #{} :internal :macro
        |&record:assoc $ %{} :CodeEntry (:doc "|internal function for record field association\nSyntax: (&record:assoc record key value & key-values)\nParams: record (record), key (any), value (any), key-values (any, variadic)\nReturns: record\nReturns new record with field associations")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal
        |&record:contains? $ %{} :CodeEntry (:doc "|internal function for checking if record contains field\nSyntax: (&record:contains? record key)\nParams: record (record), key (any)\nReturns: boolean\nReturns true if record contains field")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal
        |&record:count $ %{} :CodeEntry (:doc "|internal function for counting record fields\nSyntax: (&record:count record)\nParams: record (record)\nReturns: number\nReturns number of fields in record")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Tag
          :tags $ #{} :builtin :internal
        |&record:extend-as $ %{} :CodeEntry (:doc "|internal function for extending record as new type\nSyntax: (&record:extend-as record new-name)\nParams: record (record), new-name (keyword)\nReturns: record\nExtends record as new type with different name")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal
        |&record:from-map $ %{} :CodeEntry (:doc "|internal function for creating record from map\nSyntax: (&record:from-map name map)\nParams: name (keyword), map (map)\nReturns: record\nCreates record from map with specified name")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal
        |&record:get $ %{} :CodeEntry (:doc "|internal function for getting record field\nSyntax: (&record:get record key) or (&record:get record key default)\nParams: record (record), key (any), default (any, optional)\nReturns: any\nGets field value, returns default if field not found")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'Record 'Tag
              :return $ :: 'Optional 'Dynamic
          :tags $ #{} :builtin :internal
        |&record:get-name $ %{} :CodeEntry (:doc "|internal function for getting record name\nSyntax: (&record:get-name record)\nParams: record (record)\nReturns: keyword\nReturns name of record")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Tag)
              :args $ [] 'Tag
          :tags $ #{} :builtin :internal
        |&record:impl-traits $ %{} :CodeEntry (:doc "|internal function for record trait impl attachment\nSyntax: (&record:impl-traits record impls)\nParams: record (record), impls (any)\nReturns: record\nReturns new record with specified impls")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Record)
              :args $ [] 'Record
          :tags $ #{} :builtin :internal
        |&record:impls $ %{} :CodeEntry (:doc "|internal function for getting record impls\nSyntax: (&record:impls record)\nParams: record (record)\nReturns: any\nReturns impls of record")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'Record
              :return $ :: 'List 'Impl
          :tags $ #{} :builtin :internal
        |&record:matches? $ %{} :CodeEntry (:doc "|internal function for checking record matches\nSyntax: (&record:matches? record pattern)\nParams: record (record), pattern (any)\nReturns: boolean\nReturns true if record matches pattern")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal
        |&record:struct $ %{} :CodeEntry (:doc "|internal function for getting record source struct\nSyntax: (&record:struct record)\nParams: record (record)\nReturns: struct or nil\nReturns source struct definition of record, or nil when unavailable")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ let
                User $ defstruct User (:name 'String)
                u $ %{} User (:name |Alice)
              assert= User $ &record:struct u
          :schema $ :: 'Fn
            {}
              :args $ [] 'Record
              :return $ :: 'Optional 'Struct
          :tags $ #{} :builtin :internal
        |&record:to-map $ %{} :CodeEntry (:doc "|internal function for converting record to map\nSyntax: (&record:to-map record)\nParams: record (record)\nReturns: map\nConverts record to map")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'Record
              :return $ :: 'Map 'Tag 'Dynamic
          :tags $ #{} :builtin :internal
        |&record:with $ %{} :CodeEntry (:doc "|internal function for record with operation\nSyntax: (&record:with record key value & key-values)\nParams: record (record), key (any), value (any), key-values (any, variadic)\nReturns: record\nReturns new record with updated fields")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal
        |&reset-gensym-index! $ %{} :CodeEntry (:doc "|internal function for resetting gensym index\nSyntax: (&reset-gensym-index!)\nParams: none\nReturns: nil\nResets the global gensym counter to 0 for deterministic symbol generation")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
          :tags $ #{} :builtin :internal
        |&set:count $ %{} :CodeEntry (:doc "|internal function for counting set elements\nSyntax: (&set:count set)\nParams: set (set)\nReturns: number\nReturns number of elements in set")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] (:: 'Set 'T)
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |&set:destruct $ %{} :CodeEntry (:doc "|internal function for set destructuring\nSyntax: (&set:destruct set pattern)\nParams: set (set), pattern (any)\nReturns: set\nDestructs set according to pattern")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Set 'T)
              :generics $ [] 'T
              :return $ :: 'Optional 'Tuple
          :tags $ #{} :builtin :internal
        |&set:empty $ %{} :CodeEntry (:doc "|internal helper for set :empty method entry")
          :code $ quote
            defn &set:empty (_xs) (#{})
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Set 'T)
              :generics $ [] 'T
              :return $ :: 'Set 'T
          :tags $ #{} :internal
        |&set:empty? $ %{} :CodeEntry (:doc "|internal function for checking if set is empty\nSyntax: (&set:empty? set)\nParams: set (set)\nReturns: boolean\nReturns true if set has no elements")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] (:: 'Set 'T)
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |&set:filter $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &set:filter (xs f)
              reduce xs (#{})
                defn %&set:filter (acc x)
                  hint-fn $ {}
                    :args $ [] (:: 'Set 'T) ('T)
                    :return $ :: 'Set 'T
                  if (f x) (&include acc x) acc
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Set 'T)
                :: 'Fn $ {} (:return 'Bool)
                  :args $ [] 'T
              :generics $ [] 'T
              :return $ :: 'Set 'T
          :tags $ #{} :internal
        |&set:includes? $ %{} :CodeEntry (:doc "|internal function for checking if set includes element\nSyntax: (&set:includes? set element)\nParams: set (set), element (any)\nReturns: boolean\nReturns true if set includes element")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] (:: 'Set 'T) 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |&set:intersection $ %{} :CodeEntry (:doc "|internal function for set intersection\nSyntax: (&set:intersection set1 set2)\nParams: set1 (set), set2 (set)\nReturns: set\nReturns elements common to both sets")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Set 'T) (:: 'Set 'T)
              :generics $ [] 'T
              :return $ :: 'Set 'T
          :tags $ #{} :builtin :internal
        |&set:map $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &set:map (xs f)
              reduce xs (#{})
                defn %&set:map (acc x)
                  hint-fn $ {}
                    :args $ [] (:: 'Set 'U) 'T
                    :return $ :: 'Set 'U
                  &include acc $ f x
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Set 'T)
                :: 'Fn $ {} (:return 'U)
                  :args $ [] 'T
              :generics $ [] 'T 'U
              :return $ :: 'Set 'U
          :tags $ #{} :internal
        |&set:max $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &set:max (xs)
              &list:max $ &set:to-list xs
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Set 'T)
              :generics $ [] 'T
              :return $ :: 'Optional 'T
          :tags $ #{} :internal
        |&set:min $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &set:min (xs)
              &list:min $ &set:to-list xs
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Set 'T)
              :generics $ [] 'T
              :return $ :: 'Optional 'T
          :tags $ #{} :internal
        |&set:to-list $ %{} :CodeEntry (:doc "|internal function for converting set to list\nSyntax: (&set:to-list set)\nParams: set (set)\nReturns: list\nConverts set to list of elements")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Set 'T)
              :generics $ [] 'T
              :return $ :: 'List 'T
          :tags $ #{} :builtin :internal
        |&str $ %{} :CodeEntry (:doc "|internal function for string conversion\nSyntax: (&str value)\nParams: value (any)\nReturns: string\nConverts value to string representation")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal
        |&str-spaced $ %{} :CodeEntry (:doc "|Internal function for joining strings with spaces, used by str-spaced")
          :code $ quote
            defn &str-spaced (head? x0 & xs)
              if (&list:empty? xs)
                if head? (&str x0)
                  if (nil? x0) | $ &str:concat "| " x0
                if (some? x0)
                  &str:concat
                    if head? (&str x0) (&str:concat "| " x0)
                    &str-spaced false & xs
                  &str-spaced head? & xs
          :examples $ []
          :schema $ :: 'Fn
            {} (:rest 'T) (:return 'String)
              :args $ [] 'Bool 'T
              :generics $ [] 'T
          :tags $ #{} :internal
        |&str:compare $ %{} :CodeEntry (:doc "|internal function for string comparison\nSyntax: (&str:compare a b)\nParams: a (string), b (string)\nReturns: number\nCompares strings lexicographically, returns -1, 0, or 1")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'String 'String
          :tags $ #{} :builtin :internal
        |&str:concat $ %{} :CodeEntry (:doc "|internal function for string concatenation\nSyntax: (&str:concat a b)\nParams: a (string), b (string)\nReturns: string\nConcatenates two strings together")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'String 'String
          :tags $ #{} :builtin :internal
        |&str:contains? $ %{} :CodeEntry (:doc "|internal function for checking whether a string has a character at an index\nSyntax: (&str:contains? s index)\nParams: s (string), index (number)\nReturns: boolean\nReturns true when index is a valid character index in s")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'String 'Number
          :tags $ #{} :builtin :internal
        |&str:count $ %{} :CodeEntry (:doc "|internal function for string character count\nSyntax: (&str:count s)\nParams: s (string)\nReturns: number\nReturns number of characters in string")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'String
          :tags $ #{} :builtin :internal
        |&str:empty $ %{} :CodeEntry (:doc "|internal helper for string :empty method entry")
          :code $ quote
            defn &str:empty (_) |
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'String
          :tags $ #{} :internal
        |&str:empty? $ %{} :CodeEntry (:doc "|internal function for checking if string is empty\nSyntax: (&str:empty? s)\nParams: s (string)\nReturns: boolean\nReturns true if string has zero length")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'String
          :tags $ #{} :builtin :internal
        |&str:escape $ %{} :CodeEntry (:doc "|internal function for string escaping\nSyntax: (&str:escape s)\nParams: s (string)\nReturns: string\nEscapes special characters in string for safe output")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'String
          :tags $ #{} :builtin :internal
        |&str:find-index $ %{} :CodeEntry (:doc "|internal function for finding string index\nSyntax: (&str:find-index s pattern)\nParams: s (string), pattern (string)\nReturns: number or nil\nFinds first index of pattern in string, returns nil if not found")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'String 'String
              :return $ :: 'Optional 'Number
          :tags $ #{} :builtin :internal
        |&str:first $ %{} :CodeEntry (:doc "|internal function for getting first character\nSyntax: (&str:first s)\nParams: s (string)\nReturns: string or nil\nReturns first character of string, nil if empty")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'String
              :return $ :: 'Optional 'String
          :tags $ #{} :builtin :internal
        |&str:includes? $ %{} :CodeEntry (:doc "|internal function for checking if string includes substring\nSyntax: (&str:includes? s substring)\nParams: s (string), substring (string)\nReturns: boolean\nReturns true if string includes substring (alias for contains?)")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'String 'String
          :tags $ #{} :alias :builtin :internal
        |&str:nth $ %{} :CodeEntry (:doc "|internal function for getting nth character\nSyntax: (&str:nth s index)\nParams: s (string), index (number)\nReturns: string or nil\nReturns character at index, nil if index out of bounds")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'String 'Number
              :return $ :: 'Optional 'String
          :tags $ #{} :builtin :internal
        |&str:pad-left $ %{} :CodeEntry (:doc "|internal function for left padding string\nSyntax: (&str:pad-left s length pad-char)\nParams: s (string), length (number), pad-char (string)\nReturns: string\nPads string on left to specified length with pad character")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'String 'Number 'String
          :tags $ #{} :builtin :internal
        |&str:pad-right $ %{} :CodeEntry (:doc "|internal function for right padding string\nSyntax: (&str:pad-right s length pad-char)\nParams: s (string), length (number), pad-char (string)\nReturns: string\nPads string on right to specified length with pad character")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'String 'Number 'String
          :tags $ #{} :builtin :internal
        |&str:replace $ %{} :CodeEntry (:doc "|internal function for string replacement\nSyntax: (&str:replace s pattern replacement)\nParams: s (string), pattern (string), replacement (string)\nReturns: string\nReplaces all occurrences of pattern with replacement")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'String 'String 'String
          :tags $ #{} :builtin :internal
        |&str:rest $ %{} :CodeEntry (:doc "|internal function for getting rest of string\nSyntax: (&str:rest s)\nParams: s (string)\nReturns: string\nReturns string without first character")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'String
          :tags $ #{} :builtin :internal
        |&str:slice $ %{} :CodeEntry (:doc "|internal function for string slicing\nSyntax: (&str:slice s start end)\nParams: s (string), start (number), end (number)\nReturns: string\nExtracts substring from start to end index")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'String 'Number 'Number
          :tags $ #{} :builtin :internal
        |&struct::new $ %{} :CodeEntry (:doc "|internal function for creating struct definitions\nSyntax: (&struct::new name (field type) ...)\nParams: name (tag), field pairs (list)\nReturns: struct definition value\nCreates a struct definition with fields and type annotations")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ &struct::new :Person ([] :name :string) ([] :age :number)
          :schema $ :: 'Fn
            {} (:return 'Struct)
              :args $ [] 'Tag
          :tags $ #{} :builtin :internal
        |&struct:impl-traits $ %{} :CodeEntry (:doc "|internal function for struct trait impl attachment\nSyntax: (&struct:impl-traits struct impls)\nParams: struct (struct), impls (record)\nReturns: struct with trait implementations\nAttaches impls info to a struct definition")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Struct)
              :args $ [] 'Struct
          :tags $ #{} :builtin :internal
        |&trait::new $ %{} :CodeEntry (:doc "|internal function for creating trait values\nSyntax: (&trait::new name methods)\nParams: name (tag/symbol), methods (list of tags)\nReturns: trait\nCreates a trait definition value")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal
        |&tuple:assoc $ %{} :CodeEntry (:doc "|internal function for tuple assoc operation\nSyntax: (&tuple:assoc tuple index value)\nParams: tuple (tuple), index (number), value (any)\nReturns: new tuple with updated value\nReturns new tuple with value at index updated")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Tuple)
              :args $ [] 'Tuple 'Number 'Tag
          :tags $ #{} :builtin :internal
        |&tuple:count $ %{} :CodeEntry (:doc "|internal function for tuple count operation\nSyntax: (&tuple:count tuple)\nParams: tuple (tuple)\nReturns: number of elements\nReturns the number of elements in the tuple")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Tuple
          :tags $ #{} :builtin :internal
        |&tuple:enum $ %{} :CodeEntry (:doc "|Get the enum prototype from a tuple\nSyntax: (&tuple:enum tuple)\nParams: tuple (tuple)\nReturns: enum value or nil if not an enum tuple")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Tag)
              :args $ [] 'Tuple
          :tags $ #{} :builtin :internal
        |&tuple:enum-has-variant? $ %{} :CodeEntry (:doc "|Check if an enum has a specific variant\nSyntax: (&tuple:enum-has-variant? enum tag)\nParams: enum (enum), tag (tag)\nReturns: bool - true if variant exists")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Enum 'Tag
          :tags $ #{} :builtin :internal
        |&tuple:enum-variant-arity $ %{} :CodeEntry (:doc "|Get the arity of a variant in an enum\nSyntax: (&tuple:enum-variant-arity enum tag)\nParams: enum (enum), tag (tag)\nReturns: number - number of payload fields for the variant")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Enum 'Tag
          :tags $ #{} :builtin :internal
        |&tuple:impl-traits $ %{} :CodeEntry (:doc "|internal function for tuple trait impl attachment\nSyntax: (&tuple:impl-traits tuple new-impls)\nParams: tuple (tuple), new-impls (any)\nReturns: new tuple with updated impls\nReturns new tuple with same values but different impls")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Tuple)
              :args $ [] 'Tuple
          :tags $ #{} :builtin :internal
        |&tuple:impls $ %{} :CodeEntry (:doc "|internal function for getting tuple impls\nSyntax: (&tuple:impls tuple)\nParams: tuple (tuple)\nReturns: impls of the tuple\nReturns the impls/type identifier of the tuple")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'Tuple
              :return $ :: 'List 'Impl
          :tags $ #{} :builtin :internal
        |&tuple:nth $ %{} :CodeEntry (:doc "|internal function for tuple nth operation\nSyntax: (&tuple:nth tuple index)\nParams: tuple (tuple), index (number)\nReturns: value at index or nil\nGets the value at specified index in tuple, returns nil if out of bounds")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Tag)
              :args $ [] 'Tuple 'Number
          :tags $ #{} :builtin :internal
        |&tuple:params $ %{} :CodeEntry (:doc "|internal function for getting tuple params\nSyntax: (&tuple:params tuple)\nParams: tuple (tuple)\nReturns: list of parameter values\nReturns the parameter values of the tuple as a list")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'List)
              :args $ [] 'Tuple
          :tags $ #{} :builtin :internal
        |&tuple:validate-enum $ %{} :CodeEntry (:doc "|Validate enum tuple tag/arity if enum metadata exists\nSyntax: (&tuple:validate-enum tuple tag)\nParams: tuple (tuple), tag (tag)\nReturns: nil\nRaises error on invalid tag or arity")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ [] 'Tuple 'Tag
          :tags $ #{} :builtin :internal
        |&union $ %{} :CodeEntry (:doc "|internal function for set union\nSyntax: (&union set1 set2 & sets)\nParams: set1 (set), set2 (set), sets (set, variadic)\nReturns: set\nReturns union of all sets")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Set 'T) (:: 'Set 'T)
              :generics $ [] 'T
              :return $ :: 'Set 'T
          :tags $ #{} :builtin :internal
        |&{} $ %{} :CodeEntry (:doc "|internal function for creating maps\nSyntax: (&{} & key-value-pairs)\nParams: key-value-pairs (any, variadic)\nReturns: map\nCreates new map from key-value pairs")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal
        |' $ %{} :CodeEntry (:doc "|alias for []")
          :code $ quote (def ' [])
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :alias :internal
        |* $ %{} :CodeEntry (:doc "|Multiply numbers together")
          :code $ quote
            defn * (x & ys) (reduce ys x &*)
          :examples $ []
            quote $ assert= 6 (* 2 3)
            quote $ assert= 24 (* 2 3 4)
            quote $ assert= 2 (* 2)
            quote $ assert= 24 (* 2 3 4)
            quote $ assert= 30 (* 5 6)
            quote $ assert= 1 (* 1)
          :schema $ :: 'Fn
            {} (:rest 'Number) (:return 'Number)
              :args $ [] 'Number
        |+ $ %{} :CodeEntry (:doc "|Mathematical addition operation\\nFunction: Calculates the sum of one or more numbers\\nParams: x (number), ys (variadic args, list of numbers)\\nReturns: number - sum of all arguments\\nNotes: Supports any number of arguments, requires at least one argument")
          :code $ quote
            defn + (x & ys) (reduce ys x &+)
          :examples $ []
            quote $ quote
              assert= 6 $ + 1 2 3
            quote $ quote
              assert= 15 $ + 5 10
          :schema $ :: 'Fn
            {} (:rest 'Number) (:return 'Number)
              :args $ [] 'Number
        |- $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn - (x & ys)
              if (&list:empty? ys) (&- 0 x) (reduce ys x &-)
          :examples $ []
            quote $ assert= 5 (- 10 3 2)
            quote $ assert= -5 (- 5)
          :schema $ :: 'Fn
            {} (:rest 'Number) (:return 'Number)
              :args $ [] 'Number
        |-> $ %{} :CodeEntry (:doc "|Thread-first macro\nSyntax: (-> value step1 step2 ...)\nEvaluates the value through each step by inserting it as the first argument and returns the final result.")
          :code $ quote
            defmacro -> (base & xs)
              if (&list:empty? xs) (quasiquote ~base)
                &let
                  x0 $ &list:first xs
                  if
                    and (list? x0) (&list:empty? x0)
                    raise "|-> expects non-empty list step"
                  if
                    and
                      not $ list? x0
                      not $ thread-step? x0
                    raise $ str-spaced "|-> expects symbol/tag/fn/method or list step, got:" x0
                  if (list? x0)
                    recur
                      &list:concat
                        [] (&list:first x0) base
                        &list:rest x0
                      , & $ &list:rest xs
                    recur ([] x0 base) & $ &list:rest xs
          :examples $ []
            quote $ assert= 3 (-> 1 inc inc)
            quote $ assert= 9
              -> 2 inc $ * 3
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |->% $ %{} :CodeEntry (:doc "|pass value as `%` into several expressions")
          :code $ quote
            defmacro ->% (base & xs)
              if (&list:empty? xs) base $ let
                  tail $ last xs
                  pairs $ &list:concat
                    [] $ [] '% base
                    map (butlast xs)
                      defn %->% (x) ([] '% x)
                quasiquote $ let ~pairs ~tail
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |->> $ %{} :CodeEntry (:doc "|thread macro passing value at end of each expression")
          :code $ quote
            defmacro ->> (base & xs)
              if (&list:empty? xs) (quasiquote ~base)
                &let
                  x0 $ &list:first xs
                  if
                    and (list? x0) (&list:empty? x0)
                    raise "|->> expects non-empty list step"
                  if
                    and
                      not $ list? x0
                      not $ thread-step? x0
                    raise $ str-spaced "|->> expects symbol/tag/fn/method or list step, got:" x0
                  if (list? x0)
                    &call-spread recur (append x0 base) & $ &list:rest xs
                    &call-spread recur ([] x0 base) & $ &list:rest xs
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |/ $ %{} :CodeEntry (:doc |dividing)
          :code $ quote
            defn / (x & ys)
              if (&list:empty? ys) (&/ 1 x) (reduce ys x &/)
          :examples $ []
            quote $ / 12 3 2
            quote $ / 8
          :schema $ :: 'Fn
            {} (:rest 'Number) (:return 'Number)
              :args $ [] 'Number
        |/= $ %{} :CodeEntry (:doc "|not equal")
          :code $ quote
            defn /= (a b) (not= a b)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Dynamic 'Dynamic
        |: $ %{} :CodeEntry (:doc "|Macro sugar for tagged tuples\nExpands to `::` while passing the tag through `turn-tag`, so both keywords and bare symbols may be used.")
          :code $ quote
            defmacro : (tag & args)
              if
                not $ or (tag? tag) (symbol? tag) (string? tag)
                raise $ str-spaced "|: expects tag/symbol/string for tag, got:" tag
              quasiquote $ ::
                ~ $ turn-tag tag
                ~@ args
          :examples $ []
            quote $ assert= (:: :point 1 2) (: :point 1 2)
            quote $ assert= (:: :name |calcit) (: |name |calcit)
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |:: $ %{} :CodeEntry (:doc "|internal function for creating tuples\nSyntax: (:: impls & values)\nParams: impls (any), values (any, variable number)\nReturns: tuple with impls and values\nCreates a tuple with specified impls and values")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal
        |;nil $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro ;nil (& _body) nil
          :examples $ []
          :schema $ :: 'Macro
            {} (:return 'Unit)
              :args $ []
          :tags $ #{} :macro
        |< $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn < (x & ys)
              if
                &= 1 $ &list:count ys
                &< x $ &list:first ys
                foldl-compare ys x &<
          :examples $ []
          :schema $ :: 'Fn
            {} (:rest 'Number) (:return 'Bool)
              :args $ [] 'Number
        |<- $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro <- (& xs)
              if (&list:empty? xs) (raise "|<- expects at least 1 expression")
              quasiquote $ ->
                ~@ $ reverse xs
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :macro
        |<= $ %{} :CodeEntry (:doc "|Less than or equal comparison, supports multiple arguments")
          :code $ quote
            defn <= (x & ys)
              if
                &= 1 $ &list:count ys
                &<= x $ &list:first ys
                foldl-compare ys x &<=
          :examples $ []
            quote $ assert= true (<= 3 5)
            quote $ assert= true (<= 3 3)
            quote $ assert= true (<= 1 2 3 4)
          :schema $ :: 'Fn
            {} (:rest 'Number) (:return 'Bool)
              :args $ [] 'Number
        |= $ %{} :CodeEntry (:doc "|Equality predicate for one or more values\nReturns true only when every provided argument is equal, short-circuiting on the first mismatch.")
          :code $ quote
            defn = (x & ys)
              if
                &= 1 $ &list:count ys
                &= x $ &list:first ys
                foldl-compare ys x &=
          :examples $ []
            quote $ assert= true (= 3 3 3)
            quote $ assert= false (= 1 2)
            quote $ assert= true
              = ([] 1 2) ([] 1 2)
          :schema $ :: 'Fn
            {} (:rest 'Dynamic) (:return 'Bool)
              :args $ [] 'Dynamic
        |> $ %{} :CodeEntry (:doc "|Greater-than comparison for one or more numbers\nReturns true only when the value strictly decreases across every argument.")
          :code $ quote
            defn > (x & ys)
              if
                &= 1 $ &list:count ys
                &> x $ &list:first ys
                foldl-compare ys x &>
          :examples $ []
            quote $ assert= true (> 5 3)
            quote $ assert= false (> 3 5)
            quote $ assert= true (> 8 4 2 1)
            quote $ assert= false (> 2 2)
          :schema $ :: 'Fn
            {} (:rest 'Number) (:return 'Bool)
              :args $ [] 'Number
        |>= $ %{} :CodeEntry (:doc "|Greater-than-or-equal comparison for one or more numbers")
          :code $ quote
            defn >= (x & ys)
              if
                &= 1 $ &list:count ys
                &>= x $ &list:first ys
                foldl-compare ys x &>=
          :examples $ []
            quote $ assert= true (>= 5 3)
            quote $ assert= true (>= 5 5)
            quote $ assert= false (>= 3 5)
          :schema $ :: 'Fn
            {} (:rest 'Number) (:return 'Bool)
              :args $ [] 'Number
        |? $ %{} :CodeEntry (:doc "|internal syntax for optional argument in function definition\nSyntax: (? optional-arg) in parameter list\nParams: optional-arg (symbol)\nReturns: parameter marker\nMarks optional parameters in function definitions")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal :syntax
        |Add $ %{} :CodeEntry (:doc "|Core trait: Add")
          :code $ quote
            deftrait Add $ .add
              :: :fn $ {} (:return 'T)
                :generics $ [] 'T
                :args $ [] 'T 'T
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :trait
        |Compare $ %{} :CodeEntry (:doc "|Core trait for three-way comparison. Number and String implement it and return -1, 0, or 1.")
          :code $ quote
            deftrait Compare $ .compare
              :: :fn $ {}
                :args $ [] 'T 'T
                :generics $ [] 'T
                :return 'Number
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :trait
        |Contains $ %{} :CodeEntry (:doc "|Core trait: Contains")
          :code $ quote
            deftrait Contains $ .contains?
              :: :fn $ {}
                :args $ [] 'T 'K
                :generics $ [] 'T 'K
                :return 'Bool
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :trait
        |Countable $ %{} :CodeEntry (:doc "|Core trait: Countable")
          :code $ quote
            deftrait Countable $ .count
              :: :fn $ {} (:return :number)
                :generics $ [] 'T
                :args $ [] 'T
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :trait
        |Deserialize $ %{} :CodeEntry (:doc "|Core trait: Deserialize")
          :code $ quote
            deftrait Deserialize $ .deserialize
              :: :fn $ {} (:return 'T)
                :generics $ [] 'T
                :args $ [] :string
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :trait
        |Eq $ %{} :CodeEntry (:doc "|Core trait: Eq")
          :code $ quote
            deftrait Eq $ .eq?
              :: :fn $ {} (:return :bool)
                :generics $ [] 'T
                :args $ [] 'T 'T
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :trait
        |Len $ %{} :CodeEntry (:doc "|Core trait: Len")
          :code $ quote
            deftrait Len $ .len
              :: :fn $ {} (:return :number)
                :generics $ [] 'T
                :args $ [] 'T
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :trait
        |Mappable $ %{} :CodeEntry (:doc "|Core trait: Mappable")
          :code $ quote
            deftrait Mappable $ .map
              :: :fn $ {} (:return 'T)
                :generics $ [] 'T
                :args $ [] 'T :fn
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :trait
        |Multiply $ %{} :CodeEntry (:doc "|Core trait: Multiply")
          :code $ quote
            deftrait Multiply $ .multiply
              :: :fn $ {} (:return 'T)
                :generics $ [] 'T
                :args $ [] 'T 'T
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :trait
        |Option $ %{} :CodeEntry (:doc "|Rust-style Option enum")
          :code $ quote
            def Option $ impl-traits
              defenum Option ([] 'T) (:some 'T) (:none)
              , internal/&core-show-impl internal/&core-eq-impl OptionMappableImpl OptionMethods
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :data :internal
        |OptionMappableImpl $ %{} :CodeEntry (:doc "|Trait impl for Mappable on Option")
          :code $ quote
            defimpl OptionMappableImpl Mappable $ .map option:map
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :trait-impl
        |OptionMethods $ %{} :CodeEntry (:doc |)
          :code $ quote
            def OptionMethods $ &impl::new :OptionMethods (:: .some? option:some?) (:: .none? option:none?) (:: .unwrap-or option:unwrap-or) (:: .and-then option:and-then)
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :internal
        |Result $ %{} :CodeEntry (:doc "|Rust-style Result enum")
          :code $ quote
            def Result $ impl-traits
              defenum Result ([] 'T 'E) (:ok 'T) (:err 'E)
              , internal/&core-show-impl internal/&core-eq-impl ResultMappableImpl ResultMethods
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :data :internal
        |ResultMappableImpl $ %{} :CodeEntry (:doc "|Trait impl for Mappable on Result")
          :code $ quote
            defimpl ResultMappableImpl Mappable $ .map result:map
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :trait-impl
        |ResultMethods $ %{} :CodeEntry (:doc |)
          :code $ quote
            def ResultMethods $ &impl::new :ResultMethods (:: .ok? result:ok?) (:: .err? result:err?) (:: .unwrap-or result:unwrap-or) (:: .and-then result:and-then) (:: .map-err result:map-err)
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :internal
        |Serialize $ %{} :CodeEntry (:doc "|Core trait: Serialize")
          :code $ quote
            deftrait Serialize $ .serialize
              :: :fn $ {} (:return :string)
                :generics $ [] 'T
                :args $ [] 'T
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :trait
        |Show $ %{} :CodeEntry (:doc "|Core trait: Show")
          :code $ quote
            deftrait Show $ .show
              :: :fn $ {} (:return :string)
                :generics $ [] 'T
                :args $ [] 'T
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :trait
        |[,] $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro [,] (& body)
              &let
                xs $ &list:filter body
                  fn (x) (/= x ',)
                quasiquote $ [] ~@xs
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([])
          :tags $ #{} :macro
        |[] $ %{} :CodeEntry (:doc "|internal function for creating lists\nSyntax: ([] & elements)\nParams: elements (any, variadic)\nReturns: list\nCreates new list from provided elements")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:rest 'T)
              :args $ []
              :generics $ [] 'T
              :return $ :: 'List 'T
          :tags $ #{} :builtin :internal
        |[][] $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro [][] (& xs)
              if
                not $ and (list? xs) (every? xs list?)
                raise $ str-spaced "|[][] expects list items, got:" xs
              &let
                items $ map xs
                  fn (ys)
                    quasiquote $ [] ~@ys
                quasiquote $ [] ~@items
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([])
          :tags $ #{} :macro
        |\ $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro \ (& xs)
              if (&list:empty? xs) (raise "|\\ expects function body")
              quasiquote $ defn %\ (? % %2) ~xs
          :examples $ []
          :schema $ :: 'Macro
            {} (:return 'Fn)
              :args $ []
          :tags $ #{} :macro
        |\. $ %{} :CodeEntry (:doc "|this syntax is bared used, deprecating")
          :code $ quote
            defmacro \. (args-alias & xs)
              if
                not $ or (symbol? args-alias) (string? args-alias)
                raise $ str-spaced "|\\. expects symbol/string arg alias, got:" args-alias
              if (&list:empty? xs) (raise "|\\. expects function body")
              &let
                args $ ->% (turn-string args-alias) (split % |,) (map % turn-symbol)
                &let
                  inner-body $ if
                    &= 1 $ &list:count xs
                    &list:first xs
                    quasiquote $ &let () ~@xs
                  apply-args (inner-body args)
                    fn (body ys)
                      if (&list:empty? ys) (quasiquote ~body)
                        &let
                          a0 $ last ys
                          &let
                            code $ [] (quasiquote defn)
                              turn-symbol $ &str:concat |f_ (turn-string a0)
                              [] a0
                              , body
                            recur code $ butlast ys
          :examples $ []
          :schema $ :: 'Macro
            {} (:return 'Fn)
              :args $ [] 'Dynamic
          :tags $ #{} :deprecated :macro
        |abs $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn abs (x)
              if (&< x 0) (&- 0 x) x
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number
        |add-watch $ %{} :CodeEntry (:doc "|internal function for adding atom watchers\nSyntax: (add-watch atom key callback)\nParams: atom (atom), key (any), callback (function)\nReturns: atom\nAdds watcher function to atom")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ [] (:: 'Ref 'T) 'K 'Fn
              :generics $ [] 'T 'K
          :tags $ #{} :builtin :internal :state :watch
        |and $ %{} :CodeEntry (:doc "|Logical conjunction macro with short-circuit semantics\nReturns the first falsy value or the last truthy value, evaluating expressions left to right.")
          :code $ quote
            defmacro and (& xs)
              if (&list:empty? xs) (raise "|and expects at least 1 expression")
              &let
                item $ &list:first xs
                &let
                  rest-xs $ &list:rest xs
                  if (&list:empty? rest-xs)
                    if (list? item)
                      &let
                        v1# $ gensym |v1
                        quasiquote $ &let (~v1# ~item) (if ~v1# ~v1# false)
                      quasiquote $ if ~item ~item false
                    quasiquote $ if ~item
                      and
                        ~ $ &list:first rest-xs
                        ~@ $ &list:rest rest-xs
                      , false
          :examples $ []
            quote $ assert= false (and true false true)
            quote $ assert= |done (and true |done)
          :schema $ :: 'Dynamic
          :tags $ #{} :macro
        |any? $ %{} :CodeEntry (:doc "|checks if any element in collection satisfies the predicate function, returns true on first match, short-circuits evaluation")
          :code $ quote
            defn any? (xs f)
              foldl-shortcut xs false false $ defn %any? (acc x)
                if (f x) (:: true true) (:: false acc)
          :examples $ []
            quote $ assert= true
              any? ([] 1 2 3 4) even?
            quote $ assert= false
              any? ([] 1 3 5 7) even?
            quote $ assert= false
              any? ([]) even?
            quote $ assert= false
              any? ([] 1 2 3)
                fn (x) (> x 10)
            quote $ assert= true
              any? ([] 5 15 25)
                fn (x) (> x 10)
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Dynamic 'Fn
        |append $ %{} :CodeEntry (:doc "|internal function for appending to list\nSyntax: (append list element)\nParams: list (list), element (any)\nReturns: list\nReturns new list with element added at end")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T) 'T
              :generics $ [] 'T
              :return $ :: 'List 'T
          :tags $ #{} :builtin :internal
        |apply $ %{} :CodeEntry (:doc "|calls a function with arguments from a list, spreads the list as individual arguments")
          :code $ quote
            defn apply (f args) (f & args)
          :examples $ []
            quote $ assert= 6
              apply + $ [] 1 2 3
            quote $ assert= 10
              apply * $ [] 2 5
            quote $ assert= |abc
              apply str $ [] |a |b |c
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Fn (:: 'List 'A)
              :generics $ [] 'A
        |apply-args $ %{} :CodeEntry (:doc "|macro that applies a function to arguments, handles empty argument list specially")
          :code $ quote
            defmacro apply-args (args f)
              if
                not $ list? args
                raise $ str-spaced "|apply-args expects list args, got:" args
              if
                &= [] $ &list:first args
                quasiquote $ ~f
                  ~@ $ &list:rest args
                quasiquote $ ~f ~@args
          :examples $ []
            quote $ assert= 6
              apply-args ([] 1 2 3) +
            quote $ assert= 15
              apply-args ([] 5 10) +
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic 'Dynamic)
          :tags $ #{} :macro
        |assert $ %{} :CodeEntry (:doc "|asserts that an expression is truthy, raises an error with message if not")
          :code $ quote
            defmacro assert (message xs)
              if
                if (string? xs)
                  not $ string? message
                  , false
                quasiquote $ assert ~xs ~message
                quasiquote $ &let ()
                  if
                    not $ string? ~message
                    raise $ str-spaced "|expects 1st argument to be string, got:" ~message
                  if ~xs nil $ &let ()
                    eprintln "|Failed assertion:" $ format-to-lisp (quote ~xs)
                    raise $ ~
                      &str:concat (&str:concat message "| ") (format-to-lisp xs)
          :examples $ []
            quote $ assert "|x should be positive" (> 1 0)
            quote $ assert "|list should not be empty"
              not $ empty? ([] 1)
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic 'Dynamic)
          :tags $ #{} :control :log :macro
        |assert-detect $ %{} :CodeEntry (:doc "|asserts that a value satisfies a predicate function, raises error with details if not")
          :code $ quote
            defmacro assert-detect (f code)
              &let
                v $ gensym |v
                quasiquote $ &let (~v ~code)
                  if (~f ~v) nil $ &let () (eprintln)
                    eprintln
                      format-to-lisp $ quote ~code
                      , "|does not satisfy:"
                        format-to-lisp $ quote ~f
                        , "| <--------"
                    eprintln "|  value is:" ~v
                    raise "|Not satisfied in assertion!"
          :examples $ []
            quote $ assert-detect number? (+ 1 2)
            quote $ assert-detect list? ([] 1 2 3)
            quote $ assert-detect even? (* 2 5)
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic 'Dynamic)
          :tags $ #{} :control :log :macro
        |assert-type $ %{} :CodeEntry (:doc "|internal syntax for type assertion at preprocessing stage\nSyntax: (assert-type expr type-expr)\nParams: expr (any), type-expr (type annotation)\nReturns: evaluated result of expr\nAsserts that expr matches the given type annotation during static analysis")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal :meta :syntax
        |assert= $ %{} :CodeEntry (:doc "|asserts that two values are equal, raises error showing both values if not")
          :code $ quote
            defmacro assert= (a b)
              &let
                va $ gensym |va
                &let
                  vb $ gensym |vb
                  quasiquote $ &let (~va ~a)
                    &let (~vb ~b)
                      if (not= ~va ~vb)
                        &let () (eprintln) (eprintln "|Left: " ~va)
                          eprintln "|      " $ format-to-lisp (quote ~a)
                          eprintln |Right: ~vb
                          eprintln "|      " $ format-to-lisp (quote ~b)
                          raise "|not equal in assertion!"
          :examples $ []
            quote $ assert= 4 (+ 2 2)
            quote $ assert= |hello (str |hel |lo)
            quote $ assert= ([] 1 2 3) (range 1 4)
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic 'Dynamic)
          :tags $ #{} :control :log :macro
        |assoc $ %{} :CodeEntry (:doc "|associates a key-value pair to a collection, works on maps, lists, tuples, and records")
          :code $ quote
            defn assoc (x k v)
              if (nil? x)
                raise $ str-spaced "|assoc does not work on nil for:" k v
                if (list? x) (&list:assoc x k v) (.assoc x k v)
          :examples $ []
            quote $ assert= (&{} :a 1 :b 2)
              assoc (&{} :a 1) :b 2
            quote $ assert= ([] 10 2 3)
              assoc ([] 1 2 3) 0 10
            quote $ assert= (&{} :a 1 :b 3)
              assoc (&{} :a 1 :b 2) :b 3
          :schema $ :: 'Fn
            {} (:return 'T)
              :args $ [] 'T 'K 'V
              :generics $ [] 'T 'K 'V
        |assoc-in $ %{} :CodeEntry (:doc "|associates a value at a nested path in a data structure, creates intermediate maps if needed")
          :code $ quote
            defn assoc-in (data path v)
              list-match path
                () v
                (p0 ps)
                  &let
                    d $ either data (&{})
                    assoc d p0 $ assoc-in
                      if (contains? d p0) (get d p0) (&{})
                      , ps v
          :examples $ []
            quote $ assert=
              &{} :a $ &{} :b 1
              assoc-in (&{}) ([] :a :b) 1
            quote $ assert=
              &{} :a $ &{} :b 2
              assoc-in
                &{} :a $ &{} :b 1
                [] :a :b
                , 2
            quote $ assert=
              &{} :x $ &{} :y (&{} :z 3)
              assoc-in (&{}) ([] :x :y :z) 3
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic (:: 'List 'K) 'Dynamic
              :generics $ [] 'K
        |atom $ %{} :CodeEntry (:doc "|internal function for creating atoms\nSyntax: (atom value)\nParams: value (any)\nReturns: atom\nCreates new atom with initial value")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'T
              :generics $ [] 'T
              :return $ :: 'Ref 'T
          :tags $ #{} :builtin :internal :state
        |bit-and $ %{} :CodeEntry (:doc "|internal function for bitwise AND\nSyntax: (bit-and a b)\nParams: a (integer), b (integer)\nReturns: integer\nPerforms bitwise AND operation on two integers")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number 'Number
          :tags $ #{} :builtin :internal
        |bit-not $ %{} :CodeEntry (:doc "|internal function for bitwise NOT\nSyntax: (bit-not n)\nParams: n (integer)\nReturns: integer\nPerforms bitwise NOT operation (complement) on integer")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number
          :tags $ #{} :builtin :internal
        |bit-or $ %{} :CodeEntry (:doc "|internal function for bitwise OR\nSyntax: (bit-or a b)\nParams: a (integer), b (integer)\nReturns: integer\nPerforms bitwise OR operation on two integers")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number 'Number
          :tags $ #{} :builtin :internal
        |bit-shl $ %{} :CodeEntry (:doc "|internal function for bit shift left\nSyntax: (bit-shl n shift)\nParams: n (integer), shift (integer)\nReturns: integer\nShifts bits of n left by shift positions")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number 'Number
          :tags $ #{} :builtin :internal
        |bit-shr $ %{} :CodeEntry (:doc "|internal function for bit shift right\nSyntax: (bit-shr n shift)\nParams: n (integer), shift (integer)\nReturns: integer\nShifts bits of n right by shift positions")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number 'Number
          :tags $ #{} :builtin :internal
        |bit-xor $ %{} :CodeEntry (:doc "|internal function for bitwise XOR\nSyntax: (bit-xor a b)\nParams: a (integer), b (integer)\nReturns: integer\nPerforms bitwise XOR operation on two integers")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number 'Number
          :tags $ #{} :builtin :internal
        |blank? $ %{} :CodeEntry (:doc "|internal function for checking if string is blank\nSyntax: (blank? s)\nParams: s (string)\nReturns: boolean\nReturns true if string is empty or contains only whitespace")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'String
          :tags $ #{} :builtin :internal
        |bool? $ %{} :CodeEntry (:doc |)
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |buffer? $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn buffer? (x)
              &= (type-of x) :buffer
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T
              :generics $ [] 'T
        |butlast $ %{} :CodeEntry (:doc "|internal function for getting all but last element\nSyntax: (butlast list)\nParams: list (list)\nReturns: list\nReturns new list without the last element")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T)
              :generics $ [] 'T
              :return $ :: 'Optional (:: 'List 'T)
          :tags $ #{} :builtin :internal
        |call-w-log $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro call-w-log (f & xs)
              if
                not $ or (symbol? f) (list? f)
                raise $ str-spaced "|call-w-log expects function expression, got:" f
              let
                  v $ if
                    = :eval $ &get-calcit-running-mode
                    gensym |v
                    , '_log_tmp
                  args-value $ gensym |args-value
                quasiquote $ let
                    ~args-value $ [] ~@xs
                    ~v $ ~f & ~args-value
                  println |call:
                    format-to-lisp $ quote (call-w-log ~f ~@xs)
                    , |=> ~v
                  println "|f:   " ~f
                  println |args: ~args-value
                  ~ v
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |call-wo-log $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro call-wo-log (f & xs)
              if
                not $ or (symbol? f) (list? f)
                raise $ str-spaced "|call-wo-log expects function expression, got:" f
              quasiquote $ ~f ~@xs
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |case $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro case (item & patterns)
              if (&list:empty? patterns) (raise "|case expects at least 1 pattern")
              if
                not $ and (list? patterns) (every? patterns list?)
                raise $ str-spaced "|case expects pattern pairs in list, got:" patterns
              if
                not $ every? patterns
                  fn (pair)
                    &= 2 $ &list:count pair
                raise $ str-spaced "|case expects each pattern as pair, got:" patterns
              &let
                v $ gensym |v
                quasiquote $ &let (~v ~item) (&case ~v nil ~@patterns)
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |case-default $ %{} :CodeEntry (:doc "|Case macro variant with an explicit default branch\nEvaluates the target once, compares it against pattern/result pairs, and falls back to the provided default when no pattern matches.")
          :code $ quote
            defmacro case-default (item default & patterns)
              if (&list:empty? patterns)
                raise $ str-spaced "|Expected patterns for case-default, got empty after:" default
              if
                not $ and (list? patterns) (every? patterns list?)
                raise $ str-spaced "|case-default expects pattern pairs in list, got:" patterns
              if
                not $ every? patterns
                  fn (pair)
                    &= 2 $ &list:count pair
                raise $ str-spaced "|case-default expects each pattern as pair, got:" patterns
              &let
                v $ gensym |v
                quasiquote $ &let (~v ~item) (&case ~v ~default ~@patterns)
          :examples $ []
            quote $ assert= |two
              case-default 2 |none (1 |one) (2 |two)
            quote $ assert= |none
              case-default 3 |none $ 1 |one
            quote $ assert= |fallback
              case-default 5 |fallback (1 |one) (2 |two) (3 |three)
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic 'Dynamic)
          :tags $ #{} :macro
        |ceil $ %{} :CodeEntry (:doc "|internal function for ceiling operation\nSyntax: (ceil n)\nParams: n (number)\nReturns: number\nReturns smallest integer greater than or equal to n")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number
          :tags $ #{} :builtin :internal
        |char-from-code $ %{} :CodeEntry (:doc "|internal function for creating character from code\nSyntax: (char-from-code code)\nParams: code (number)\nReturns: string\nCreates character from Unicode code point")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'Number
          :tags $ #{} :builtin :internal
        |cirru-quote? $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn cirru-quote? (x)
              &= (type-of x) :cirru-quote
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T
              :generics $ [] 'T
        |concat $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn concat (& args)
              list-match args
                () $ []
                (a0 as) (&list:concat a0 & as)
          :examples $ []
            quote $ assert= ([] 1 2 3 4 5)
              concat ([] 1 2) ([] 3 4) ([] 5)
          :schema $ :: 'Fn
            {}
              :args $ []
              :generics $ [] 'T
              :rest $ :: 'List 'T
              :return $ :: 'List 'T
        |cond $ %{} :CodeEntry (:doc "|Multi-branch conditional macro. Evaluates condition/result pairs in order and returns the first truthy branch; use `true` as a default guard.")
          :code $ quote
            defmacro cond (& pairs)
              if (&list:empty? pairs) (raise "|cond expects at least 1 (condition branch) pair")
              &let
                pair $ &list:first pairs
                if
                  not $ and (list? pair)
                    &= 2 $ &list:count pair
                  raise $ str-spaced "|cond expects a pair, got:" pair
                &let
                  else $ &list:rest pairs
                  &let
                    expr $ &list:nth pair 0
                    &let
                      branch $ &list:nth pair 1
                      if
                        if (empty? else) (= true expr) false
                        , branch $ quasiquote
                          if ~expr ~branch $ ~
                            if (&list:empty? else) (;nil)
                              quasiquote $ cond ~@else
          :examples $ []
            quote $ assert= :small
              cond
                  &< 2 1
                  , :nope
                (&< 2 5) :small
                true :fallback
            quote $ assert= :fallback
              cond (false :branch) (true :fallback)
          :schema $ :: 'Dynamic
          :tags $ #{} :macro
        |conj $ %{} :CodeEntry (:doc "|Appends values to the end of a list, returning a new list\nSupports adding multiple values by chaining additional arguments.")
          :code $ quote
            defn conj (xs y0 & ys)
              if (empty? ys) (append xs y0)
                recur (append xs y0) & ys
          :examples $ []
            quote $ assert= ([] 1 2 3)
              conj ([] 1 2) 3
            quote $ assert= ([] 1 2 3 4)
              conj ([] 1) 2 3 4
          :schema $ :: 'Fn
            {} (:rest 'T)
              :args $ [] (:: 'List 'T) 'T
              :generics $ [] 'T
              :return $ :: 'List 'T
        |contains-in? $ %{} :CodeEntry (:doc "|Checks whether a nested path exists within maps, records, tuples, or lists. Returns true only when every hop succeeds.")
          :code $ quote
            defn contains-in? (xs path)
              list-match path
                () true
                (p0 ps)
                  cond
                      list? xs
                      if
                        and (number? p0) (&list:contains? xs p0)
                        recur (nth xs p0) ps
                        , false
                    (map? xs)
                      if (&map:contains? xs p0)
                        recur (&map:get xs p0) ps
                        , false
                    (record? xs)
                      if (&record:contains? xs p0)
                        recur (&record:get xs p0) ps
                        , false
                    (tuple? xs)
                      if
                        and (&>= p0 0)
                          &< p0 $ &tuple:count xs
                        recur (&tuple:nth xs p0) ps
                        , false
                    true false
          :examples $ []
            quote $ assert= true
              contains-in?
                {} $ :profile
                  {} $ :name |calcit
                [] :profile :name
            quote $ assert= false
              contains-in?
                {} $ :profile
                  {} $ :name |calcit
                [] :profile :missing
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T (:: 'List 'K)
              :generics $ [] 'T 'K
        |contains-symbol? $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn contains-symbol? (xs y)
              if (list? xs)
                apply-args (xs)
                  defn %contains-symbol? (body)
                    list-match body
                      () false
                      (b0 bs)
                        if (contains-symbol? b0 y) true $ recur bs
                &= xs y
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T 'Symbol
              :generics $ [] 'T
        |contains? $ %{} :CodeEntry (:doc "|Checks whether a collection contains a key or index at the current level. Supports lists, tuples, maps, and records while treating nil as false.")
          :code $ quote
            defn contains? (x k)
              if (nil? x) false $ if (list? x) (&list:contains? x k) (.contains? x k)
          :examples $ []
            quote $ assert= true
              contains? ([] :a :b) 1
            quote $ assert= true
              contains?
                {} $ :a 1
                , :a
            quote $ assert= false (contains? nil :missing)
            quote $ assert= true
              contains? (#{} 1 2 3) 2
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T 'K
              :generics $ [] 'T 'K
        |cos $ %{} :CodeEntry (:doc "|internal function for cosine\nSyntax: (cos n)\nParams: n (number, radians)\nReturns: number\nReturns cosine of angle in radians")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number
          :tags $ #{} :builtin :internal
        |count $ %{} :CodeEntry (:doc "|Counts elements in a collection or string\nNil input returns 0; otherwise delegates to the underlying data structure's counter.")
          :code $ quote
            defn count (x)
              if (nil? x) 0 $ if (list? x) (&list:count x) (.count x)
          :examples $ []
            quote $ assert= 4
              count $ [] 1 2 3 4
            quote $ assert= 5 (count |hello)
            quote $ assert= 0 (count nil)
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] (:: 'Optional 'T)
              :generics $ [] 'T
        |cpu-time $ %{} :CodeEntry (:doc "|internal function for getting CPU time\nSyntax: (cpu-time)\nParams: none\nReturns: number\nReturns current CPU time in milliseconds for performance measurement")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ []
          :tags $ #{} :builtin :internal :io
        |data-definition-form $ %{} :CodeEntry (:doc "|Normalize wrapped forms used by data-definition macros")
          :code $ quote
            defn data-definition-form (entry)
              if
                and (list? entry)
                  not $ empty? entry
                  &= [] $ &list:first entry
                &list:rest entry
                if
                  and (list? entry)
                    &= 1 $ count entry
                    list? $ &list:first entry
                  &list:first entry
                  , entry
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic
        |data-definition-malformed-nesting? $ %{} :CodeEntry (:doc "|Detect suspicious nested forms that usually come from wrong indentation in data-definition macros")
          :code $ quote
            defn data-definition-malformed-nesting? (form)
              if
                and (list? form)
                  &= 1 $ count form
                  list? $ &list:first form
                &let
                  child $ data-definition-form (&list:first form)
                  and (list? child)
                    not $ empty? child
                    or
                      tag? $ &list:first child
                      and
                        not $ tag? (&list:first child)
                        every? (&list:rest child)
                          fn (bound)
                            &let
                              items $ data-definition-form bound
                              and (list? items)
                                &= 2 $ count items
                , false
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Dynamic
        |data-definition-where-form? $ %{} :CodeEntry (:doc "|Detect optional where-map form after generic parameters in data-definition macros")
          :code $ quote
            defn data-definition-where-form? (tail-forms)
              if (empty? tail-forms) false $ &let
                candidate $ data-definition-form (&list:first tail-forms)
                and (list? candidate)
                  not $ empty? candidate
                  not $ tag? (&list:first candidate)
                  every? (&list:rest candidate)
                    fn (bound)
                      &let
                        items $ data-definition-form bound
                        and (list? items)
                          &= 2 $ count items
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Dynamic
        |dec $ %{} :CodeEntry (:doc "|Decrements a number by 1")
          :code $ quote
            defn dec (x) (&- x 1)
          :examples $ []
            quote $ assert= 4 (dec 5)
            quote $ assert= -1 (dec 0)
            quote $ assert= -4 (dec -3)
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number
        |def $ %{} :CodeEntry (:doc "|special macro to expose value to definition")
          :code $ quote
            defmacro def (_name x) x
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic 'Dynamic)
          :tags $ #{} :macro
        |defatom $ %{} :CodeEntry (:doc "|internal syntax for defining referenced state\nSyntax: (defatom name initial-value)\nParams: name (symbol), initial-value (any)\nReturns: atom definition\nDefines a mutable reference with initial value")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ ; defatom *my-atom
              {} $ :a 1
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic 'Dynamic
          :tags $ #{} :builtin :internal :state :syntax
        |defenum $ %{} :CodeEntry (:doc "|macro for defining enums\nSyntax: (defenum Name [('T 'E)] (:variant type...) ...)\nParams: Name (symbol/tag), optional generics list, variants (tag + payload types)\nReturns: enum prototype value\nExpands to &enum::new")
          :code $ quote
            defmacro defenum (name & variants)
              assert "|defenum expects name as tag/symbol" $ or (tag? name) (symbol? name)
              assert "|defenum expects variants in list" $ and (list? variants) (every? variants list?)
              &let
                first-variant $ if (empty? variants) (;nil) (&list:first variants)
                &let
                  generics $ if (list? first-variant)
                    if
                      and
                        not $ empty? first-variant
                        not $ tag? (&list:first first-variant)
                      if
                        &= [] $ &list:first first-variant
                        &list:rest first-variant
                        , first-variant
                    , []
                  &let
                    tail-forms $ if (empty? generics) variants (&list:rest variants)
                    &let
                      has-where-form? $ data-definition-where-form? tail-forms
                      &let
                        where-form $ if has-where-form?
                          data-definition-form $ &list:first tail-forms
                          {}
                        &let
                          variant-forms $ if has-where-form? (&list:rest tail-forms) tail-forms
                          assert "|defenum expects each variant as (:tag & payloads); check indentation if one variant was nested under another" $ every? variant-forms
                            fn (variant)
                              &let
                                items $ data-definition-form variant
                                and
                                  &>= (count items) 1
                                  tag? $ &list:first items
                          assert "|defenum found malformed nested payload syntax; check indentation around variants" $ every? variant-forms
                            fn (variant)
                              &let
                                items $ data-definition-form variant
                                every? (&list:rest items)
                                  fn (payload-form)
                                    not $ data-definition-malformed-nesting? payload-form
                          &let
                            normalized $ map variant-forms
                              fn (variant)
                                &let
                                  items $ data-definition-form variant
                                  &let
                                    variant-tag $ &list:first items
                                    &let
                                      payload-forms $ map (&list:rest items)
                                        fn (t)
                                          if (list? t)
                                            quasiquote $ quote (~ t)
                                            , t
                                      quasiquote $ [] (~ variant-tag) (~@ payload-forms)
                            if (empty? generics)
                              if has-where-form?
                                quasiquote $ &enum::new
                                  ~ $ turn-tag name
                                  ~ where-form
                                  ~@ normalized
                                quasiquote $ &enum::new
                                  ~ $ turn-tag name
                                  ~@ normalized
                              if has-where-form?
                                quasiquote $ &enum::new
                                  ~ $ turn-tag name
                                  [] ~@generics
                                  ~ where-form
                                  ~@ normalized
                                quasiquote $ &enum::new
                                  ~ $ turn-tag name
                                  [] ~@generics
                                  ~@ normalized
          :examples $ []
            quote $ defenum Result ([] 'T 'E) (:ok 'T) (:err 'E)
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |defimpl $ %{} :CodeEntry (:doc "|macro for defining trait implementation values\nSyntax: (defimpl ImplName Trait (.method value) ...)\nParams: ImplName (symbol), Trait (symbol), method pairs\nReturns: impl value\nNotes: new code passes raw symbols; tag arguments remain only as legacy compatibility for originless method bags. This macro does not attach an impl to a target type/value; use `impl-traits` separately.\nExpands to &impl::new")
          :code $ quote
            defmacro defimpl (name trait & pairs)
              if
                not $ or (tag? name) (symbol? name)
                raise $ str-spaced "|defimpl misuse. Expected: first argument is impl name symbol (legacy tag accepted). Actual:" name "|Fix: rewrite as (defimpl ImplName Trait ...)."
              if
                not $ or (tag? trait) (symbol? trait)
                raise $ str-spaced "|defimpl misuse. Expected: second argument is trait symbol (legacy tag accepted). Actual:" trait "|Fix: rewrite as (defimpl ImplName Trait ...)."
              quasiquote $ def ~name
                &impl::new
                  ~ $ if (tag? trait) (turn-tag trait) trait
                  ~@ $ if (every? pairs list?)
                    do
                      assert "|defimpl expects method pairs" $ and (list? pairs) (every? pairs list?)
                      assert "|defimpl expects (:method value) pairs" $ every? pairs
                        fn (pair)
                          &let
                            items $ if
                              &= [] $ &list:first pair
                              &list:rest pair
                              if
                                &= (quote ::) (&list:first pair)
                                &list:rest pair
                                , pair
                            and
                              &= 2 $ count items
                              or
                                tag? $ &list:first items
                                &= :method $ type-of (&list:first items)
                      map pairs $ fn (pair)
                        &let
                          items $ if
                            &= [] $ &list:first pair
                            &list:rest pair
                            if
                              &= (quote ::) (&list:first pair)
                              &list:rest pair
                              , pair
                          do
                            assert "|defimpl expects (:method value) pairs" $ &= 2 (count items)
                            let
                                k0 $ &list:first items
                                v0 $ &list:nth items 1
                                key $ if (tag? k0) k0
                                  if
                                    &= :method $ type-of k0
                                    let
                                        s $ format-to-lisp k0
                                      turn-tag $ &str:slice s 1 (count s)
                                    raise $ str-spaced "|defimpl expects method key as :tag or .method, got:" k0
                              quasiquote $ [] ~key ~v0
                    do
                      assert "|defimpl expects even number of items" $ &= 0
                        &number:rem (count pairs) 2
                      map (section-by pairs 2)
                        fn (pair)
                          &let
                            items $ if
                              &= [] $ &list:first pair
                              &list:rest pair
                              if
                                &= (quote ::) (&list:first pair)
                                &list:rest pair
                                , pair
                            do
                              assert "|defimpl expects (:method value) pairs" $ &= 2 (count items)
                              let
                                  k0 $ &list:first items
                                  v0 $ &list:nth items 1
                                  key $ if (tag? k0) k0
                                    if
                                      &= :method $ type-of k0
                                      let
                                          s $ format-to-lisp k0
                                        turn-tag $ &str:slice s 1 (count s)
                                      raise $ str-spaced "|defimpl expects method key as :tag or .method, got:" k0
                                quasiquote $ [] ~key ~v0
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic 'Dynamic)
          :tags $ #{} :macro
        |defmacro $ %{} :CodeEntry (:doc "|internal syntax for defining macros\nSyntax: (defmacro name [args] body)\nParams: name (symbol), args (list of symbols), body (expression)\nReturns: macro definition\nDefines a macro that transforms code at compile time")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ do
              defmacro identity-macro (x) (quasiquote ~x)
              assert= 4 $ identity-macro (+ 2 2)
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic 'Dynamic 'Dynamic
          :tags $ #{} :builtin :internal :syntax
        |defn $ %{} :CodeEntry (:doc "|internal syntax for defining functions\nSyntax: (defn name [args] body)\nParams: name (symbol), args (list of symbols), body (expression)\nReturns: function definition\nDefines a named function with parameters and body expression")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ defn my-add (p1 p2) (+ p1 p2)
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic 'Dynamic 'Dynamic
          :tags $ #{} :builtin :internal :syntax
        |defn-w-log $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro defn-w-log (f-name args & body)
              if
                not $ symbol? f-name
                raise $ str-spaced "|defn-w-log expects function name symbol, got:" f-name
              if
                not $ list? args
                raise $ str-spaced "|defn-w-log expects args in list, got:" args
              if (&list:empty? body) (raise "|defn-w-log expects function body")
              quasiquote $ defn ~f-name ~args
                &let
                  ~f-name $ defn ~f-name ~args ~@body
                  call-w-log ~f-name ~@args
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic 'Dynamic)
          :tags $ #{} :macro
        |defn-wo-log $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro defn-wo-log (f-name args & body)
              if
                not $ symbol? f-name
                raise $ str-spaced "|defn-wo-log expects function name symbol, got:" f-name
              if
                not $ list? args
                raise $ str-spaced "|defn-wo-log expects args in list, got:" args
              if (&list:empty? body) (raise "|defn-wo-log expects function body")
              quasiquote $ defn ~f-name ~args ~@body
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic 'Dynamic)
          :tags $ #{} :macro
        |defstruct $ %{} :CodeEntry (:doc "|macro for defining record structs\nSyntax: (defstruct Name [('T 'S)] :field :type ...)\nParams: Name (symbol/tag), optional generics list, field pairs (tag + type)\nReturns: struct definition value\nExpands to &struct::new")
          :code $ quote
            defmacro defstruct (name & pairs)
              assert "|defstruct expects name as tag/symbol" $ or (tag? name) (symbol? name)
              assert "|defstruct expects pairs in list" $ and (list? pairs) (every? pairs list?)
              &let
                first-pair $ if (empty? pairs) (;nil) (&list:first pairs)
                &let
                  generics $ if (list? first-pair)
                    if
                      and
                        not $ empty? first-pair
                        not $ tag? (&list:first first-pair)
                      if
                        &= [] $ &list:first first-pair
                        &list:rest first-pair
                        , first-pair
                    , []
                  &let
                    tail-forms $ if (empty? generics) pairs (&list:rest pairs)
                    &let
                      has-where-form? $ data-definition-where-form? tail-forms
                      &let
                        where-form $ if has-where-form?
                          data-definition-form $ &list:first tail-forms
                          {}
                        &let
                          field-pairs $ if has-where-form? (&list:rest tail-forms) tail-forms
                          assert "|defstruct expects each field as (:field type); check indentation if one field was nested under another" $ every? field-pairs
                            fn (pair)
                              &let
                                items $ data-definition-form pair
                                and
                                  &= 2 $ count items
                                  tag? $ &list:first items
                          assert "|defstruct found malformed nested field syntax; check indentation around field pairs" $ every? field-pairs
                            fn (pair)
                              &let
                                items $ data-definition-form pair
                                not $ data-definition-malformed-nesting? (last items)
                          &let
                            normalized $ map field-pairs
                              fn (pair)
                                &let
                                  items $ data-definition-form pair
                                  &let
                                    field-name $ &list:first items
                                    &let
                                      type-form $ last items
                                      if (list? type-form)
                                        if
                                          and
                                            &= 2 $ count type-form
                                            syntax? $ &list:first type-form
                                          if (includes? generics type-form)
                                            quasiquote $ [] (~ field-name)
                                              quote $ ~ type-form
                                            quasiquote $ [] (~ field-name) (~ type-form)
                                          quasiquote $ [] (~ field-name)
                                            quote $ ~ type-form
                                        quasiquote $ [] (~ field-name) (~ type-form)
                            if (empty? generics)
                              if has-where-form?
                                quasiquote $ &struct::new
                                  ~ $ turn-tag name
                                  ~ where-form
                                  ~@ normalized
                                quasiquote $ &struct::new
                                  ~ $ turn-tag name
                                  ~@ normalized
                              if has-where-form?
                                quasiquote $ &struct::new
                                  ~ $ turn-tag name
                                  [] ~@generics
                                  ~ where-form
                                  ~@ normalized
                                quasiquote $ &struct::new
                                  ~ $ turn-tag name
                                  [] ~@generics
                                  ~@ normalized
          :examples $ []
            quote $ defstruct Person (:name 'String) (:age 'Number)
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |deftrait $ %{} :CodeEntry (:doc "|macro for defining traits\nSyntax: (deftrait Name (.method (:: :fn $ {} (:args [...]) (:return t))) ...)\nParams: Name (symbol/tag), methods (list of (tag type))\nNotes: use :fn (tag) for DynFn when signature is intentionally omitted\nReturns: trait definition value\nExpands to &trait::new")
          :code $ quote
            defmacro deftrait (name & methods)
              assert "|deftrait expects (method type) pairs" $ every? methods list?
              &let
                normalized $ map methods
                  fn (entry)
                    &let
                      items $ if
                        &= [] $ &list:first entry
                        &list:rest entry
                        , entry
                      do
                        assert "|deftrait expects (method type) pairs" $ &= 2 (count items)
                        let
                            m0 $ &list:first items
                            t0 $ &list:nth items 1
                            k0 $ if (tag? m0) m0
                              if
                                &= :method $ type-of m0
                                let
                                    s $ format-to-lisp m0
                                  turn-tag $ &str:slice s 1 (count s)
                                raise $ str-spaced "|deftrait expects method key as :tag or .method, got:" m0
                            t1 $ internal/normalize-trait-type t0
                          quasiquote $ [] ~k0 (quote ~t1)
                quasiquote $ def ~name
                  &trait::new
                    ~ $ turn-tag name
                    [] ~@normalized
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |deftype-slot $ %{} :CodeEntry (:doc "|Declare a named compile-time type slot supplied by a library. Syntax: (deftype-slot :slot-name). Applications should bind the slot for each entry with cr config set-type-slot; an unbound slot falls back to :dynamic.")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ [] 'Tag
          :tags $ #{} :builtin :internal :meta :state :syntax
        |deref $ %{} :CodeEntry (:doc "|Reads the current value stored in a reference\nSupports Calcit atoms as well as other host structures that implement deref.")
          :code $ quote
            defn deref (*a)
              if (ref? *a) (&atom:deref *a) (.deref *a)
          :examples $ []
            quote $ do (defatom *state 1)
              assert= 1 $ deref *state
            quote $ do (defatom *counter 0) (reset! *counter 5)
              assert= 5 $ deref *counter
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal :state
        |destruct-list $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn destruct-list (xs)
              if (empty? xs) (:: :none)
                :: :some (nth xs 0) (&list:slice xs 1)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Tuple)
              :args $ [] (:: 'List 'T)
              :generics $ [] 'T
        |destruct-map $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn destruct-map (xs)
              &let
                pair $ &map:destruct xs
                if (nil? pair) (:: :none) (:: :some & pair)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Tuple)
              :args $ [] (:: 'Map 'K 'V)
              :generics $ [] 'K 'V
        |destruct-set $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn destruct-set (xs)
              &let
                pair $ &set:destruct xs
                if (nil? pair) (:: :none)
                  :: :some (nth pair 0) (nth pair 1)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Tuple)
              :args $ [] (:: 'Set 'T)
              :generics $ [] 'T
        |destruct-str $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn destruct-str (s)
              if (&= s |) (:: :none)
                :: :some (nth s 0) (&str:slice s 1)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Tuple)
              :args $ [] 'String
        |difference $ %{} :CodeEntry (:doc "|Returns the set difference of base and all other sets")
          :code $ quote
            defn difference (base & xs)
              reduce xs base $ fn (acc item) (&difference acc item)
          :examples $ []
            quote $ assert= (#{} 1)
              difference (#{} 1 2 3) (#{} 2 3 4)
            quote $ assert= (#{} 1 2)
              difference (#{} 1 2 3 4) (#{} 3 4 5)
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Set 'T)
              :generics $ [] 'T
              :rest $ :: 'Set 'T
              :return $ :: 'Set 'T
        |dissoc $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn dissoc (x & args)
              if (nil? x) x $ if (list? x) (&list:dissoc x & args)
                if (map? x) (&map:dissoc x & args) (.dissoc x & args)
          :examples $ []
          :schema $ :: 'Fn
            {} (:rest 'K) (:return 'T)
              :args $ [] 'T
              :generics $ [] 'T 'K
        |dissoc-in $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn dissoc-in (data path)
              list-match path
                () $ ;nil
                (p0 ps)
                  if
                    &= 1 $ &list:count path
                    dissoc data p0
                    assoc data p0 $ dissoc-in (get data p0) ps
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic (:: 'List 'K)
              :generics $ [] 'K
        |distinct $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn distinct (x) (&list:distinct x)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T)
              :generics $ [] 'T
              :return $ :: 'List 'T
        |do $ %{} :CodeEntry (:doc "|Evaluates expressions sequentially and returns the last result\nUseful for grouping side effects or multiple steps where only the final value matters.")
          :code $ quote
            defmacro do (& body)
              ; println |body: $ format-to-lisp body
              if (empty? body) (raise "|empty do is not okay")
              quasiquote $ &let () (~@ body)
          :examples $ []
            quote $ assert= 3
              do (inc 1) (+ 1 2)
            quote $ assert= |world
              do (str |hello) (str |world)
          :schema $ :: 'Dynamic
          :tags $ #{} :macro
        |drop $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn drop (xs n)
              slice xs n $ &list:count xs
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T) 'Number
              :generics $ [] 'T
              :return $ :: 'List 'T
        |each $ %{} :CodeEntry (:doc "|Iterate over a collection and apply function f for side effects, returns nil")
          :code $ quote
            defn each (xs f)
              foldl xs (;nil)
                defn %each (_acc x) (f x)
          :examples $ []
            quote $ assert= nil
              each ([] 1 2 3)
                fn (x) (&+ x 1)
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ [] 'Dynamic
                :: 'Fn $ {} (:return 'R)
                  :args $ [] 'T
              :generics $ [] 'T 'R
        |either $ %{} :CodeEntry (:doc "|Returns the first non-nil value among its arguments\nBehaves like a nil-coalescing macro: only nil triggers evaluation of subsequent branches, so false is preserved as a value.")
          :code $ quote
            defmacro either (& xs)
              if (&list:empty? xs) (raise "|either expects at least 1 expression")
              &let
                item $ &list:first xs
                &let
                  rest-xs $ &list:rest xs
                  if (&list:empty? rest-xs) item $ if (list? item)
                    &let
                      v1# $ gensym |v1
                      quasiquote $ &let (~v1# ~item)
                        if (nil? ~v1#)
                          either
                            ~ $ &list:first rest-xs
                            ~@ $ &list:rest rest-xs
                          ~ v1#
                    quasiquote $ if (nil? ~item)
                      either
                        ~ $ &list:first rest-xs
                        ~@ $ &list:rest rest-xs
                      ~ item
          :examples $ []
            quote $ assert= 42 (either nil 42 nil)
            quote $ assert= false (either false true)
            quote $ assert= |backup (either nil nil |backup)
          :schema $ :: 'Dynamic
          :tags $ #{} :macro
        |empty $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn empty (x)
              if (nil? x) x $ if (list? x) ([]) (.empty x)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Optional 'T)
              :generics $ [] 'T
              :return $ :: 'Optional 'T
        |empty? $ %{} :CodeEntry (:doc "|Checks whether a collection or string is empty\nNil values are considered empty, otherwise delegates to the underlying data structure.")
          :code $ quote
            defn empty? (x)
              if (nil? x) true $ if (list? x) (&list:empty? x) (.empty? x)
          :examples $ []
            quote $ assert= true
              empty? $ []
            quote $ assert= false
              empty? $ [] 1
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T
              :generics $ [] 'T
        |ends-with? $ %{} :CodeEntry (:doc "|internal function for checking string suffix\nSyntax: (ends-with? s suffix)\nParams: s (string), suffix (string)\nReturns: boolean\nReturns true if string ends with suffix")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'String 'String
          :tags $ #{} :builtin :internal
        |enum? $ %{} :CodeEntry (:doc "|Predicate that checks whether a value is an enum definition.")
          :code $ quote
            defn enum? (x)
              &= (type-of x) :enum
          :examples $ []
            quote $ assert= true
              enum? $ defenum Result (:ok) (:err 'String)
            quote $ assert= false
              enum? $ :: :ok 1
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T
              :generics $ [] 'T
        |eval $ %{} :CodeEntry (:doc "|internal syntax for evaluating code at runtime\nSyntax: (eval expr)\nParams: expr (quoted code)\nReturns: result of evaluation\nEvaluates quoted code in current environment")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal :interop :syntax
        |even? $ %{} :CodeEntry (:doc "|check if number is even?")
          :code $ quote
            defn even? (n)
              &= 0 $ &number:rem n 2
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Number
        |every? $ %{} :CodeEntry (:doc "|Checks whether every element of a collection satisfies a predicate, short-circuiting on the first failure.")
          :code $ quote
            defn every? (xs f)
              foldl-shortcut xs true true $ defn %every? (acc x)
                if (f x) (:: false acc) (:: true false)
          :examples $ []
            quote $ assert= true
              every? ([] 2 4 6)
                defn %even (x)
                  &= 0 $ .rem x 2
            quote $ assert= false
              every? ([] 1 2 3)
                defn %gt1 (x) (&> x 1)
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Dynamic
                :: 'Fn $ {} (:return 'Bool)
                  :args $ [] 'T
              :generics $ [] 'T
        |exclude $ %{} :CodeEntry (:doc "|Removes values from a collection by repeatedly calling `&exclude` for each provided item.")
          :code $ quote
            defn exclude (base & xs)
              reduce xs base $ fn (acc item) (&exclude acc item)
          :examples $ []
          :schema $ :: 'Fn
            {} (:rest 'T)
              :args $ [] (:: 'Set 'T)
              :generics $ [] 'T
              :return $ :: 'Set 'T
        |field-match $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro field-match (value & body)
              if (&list:empty? body) (raise "|field-match expected patterns for matching")
                if (list? value)
                  &let
                    v# $ gensym |v
                    quasiquote $ &let (~v# ~value)
                      assert "|expected map value to match" $ map? ~v#
                      internal/&field-match-internal ~v# ~@body
                  quasiquote $ &let ()
                    assert "|expected map value to match" $ map? ~value
                    internal/&field-match-internal ~value ~@body
          :examples $ []
          :schema $ :: 'Macro
            {}
              :args $ [] (:: 'Map 'K 'V)
              :generics $ [] 'K 'V
          :tags $ #{} :macro
        |filter $ %{} :CodeEntry (:doc "|Builds a new collection containing only the elements where the predicate returns truthy, preserving the original collection type when possible.")
          :code $ quote
            defn filter (xs f)
              if (nil? xs) xs $ if (list? xs) (&list:filter xs f) (.filter xs f)
          :examples $ []
            quote $ assert= ([] 2 4)
              filter ([] 1 2 3 4 5)
                defn %even? (x)
                  &= 0 $ .rem x 2
            quote $ assert= ([] |bb |ccc)
              filter ([] |a |bb |ccc)
                defn %long? (s)
                  &> (&str:count s) 1
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic 'Fn
        |filter-not $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn filter-not (xs f)
              filter xs $ defn %filter-not (x)
                not $ f x
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T)
                :: 'Fn $ {} (:return 'Bool)
                  :args $ [] 'T
              :generics $ [] 'T
              :return $ :: 'List 'T
        |find $ %{} :CodeEntry (:doc "|Find the first element in a collection that satisfies the predicate f")
          :code $ quote
            defn find (xs f)
              foldl-shortcut xs 0 (;nil)
                defn %find (_acc x)
                  if (f x) (:: true x)
                    :: false $ ;nil
          :examples $ []
            quote $ assert= 4
              find ([] 1 2 3 4 5)
                fn (x) (&> x 3)
            quote $ assert= nil
              find ([] 1 2 3)
                fn (x) (&> x 10)
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T)
                :: 'Fn $ {} (:return 'Bool)
                  :args $ [] 'T
              :generics $ [] 'T
              :return $ :: 'Optional 'T
        |find-index $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn find-index (xs f)
              foldl-shortcut xs 0 (;nil)
                defn %find-index (idx x)
                  if (f x) (:: true idx)
                    :: false $ &+ 1 idx
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T)
                :: 'Fn $ {} (:return 'Bool)
                  :args $ [] 'T
              :generics $ [] 'T
              :return $ :: 'Optional 'Number
        |first $ %{} :CodeEntry (:doc "|Returns the first element of a list, tuple, string, or other sequential structure\nNil inputs return nil, and empty collections also produce nil.")
          :code $ quote
            defn first (x)
              if (nil? x) x $ if (list? x) (&list:first x) (.first x)
          :examples $ []
            quote $ assert= 1
              first $ [] 1 2 3
            quote $ assert= |h (first |hello)
            quote $ assert= nil (first nil)
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Optional 'T)
              :generics $ [] 'T
              :return $ :: 'Optional 'Dynamic
        |flipped $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro flipped (f & args)
              quasiquote $ ~f
                ~@ $ reverse args
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |floor $ %{} :CodeEntry (:doc "|internal function for floor operation\nSyntax: (floor n)\nParams: n (number)\nReturns: number\nReturns largest integer less than or equal to n")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number
          :tags $ #{} :builtin :internal
        |fn $ %{} :CodeEntry (:doc "|macro for anonymous functions\nSyntax: (fn (args...) body...)\nParams: args (parameter list), body (expressions)\nReturns: anonymous function\nCreates an anonymous function, shorter than defn")
          :code $ quote
            defmacro fn (args & body)
              if
                not $ list? args
                raise $ str-spaced "|fn expects args in list, got:" args
              if (&list:empty? body)
                quasiquote $ defn f% ~args nil
                quasiquote $ defn f% ~args ~@body
          :examples $ []
            quote $ map ([] 1 2 3)
              fn (x) (* x 2)
            quote $ filter ([] 1 2 3 4 5)
              fn (n) (> n 2)
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |fn? $ %{} :CodeEntry (:doc "|Check if a value is a function")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ assert= true (fn? inc)
            quote $ assert= false (fn? 123)
            quote $ assert= false (fn? |text)
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |foldl $ %{} :CodeEntry (:doc "|internal function for left fold\nSyntax: (foldl list initial reducer)\nParams: list (list), initial (any), reducer (function)\nReturns: any\nFolds list from left with reducer function and initial value")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'U)
              :args $ [] (:: 'List 'T) 'U
                :: 'Fn $ {} (:return 'U)
                  :args $ [] 'U 'T
              :generics $ [] 'T 'U
          :tags $ #{} :builtin :internal
        |foldl' $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn foldl' (xs acc f)
              list-match xs
                () acc
                (x0 xss)
                  recur xss (f acc x0) f
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'U)
              :args $ [] (:: 'List 'T) 'U
                :: 'Fn $ {} (:return 'U)
                  :args $ [] 'U 'T
              :generics $ [] 'T 'U
        |foldl-compare $ %{} :CodeEntry (:doc "|Helper used by comparison operators to ensure a relation holds across an entire list, short-circuiting on the first failure.")
          :code $ quote
            defn foldl-compare (xs acc f)
              if (&list:empty? xs) true $ if
                f acc $ &list:first xs
                recur (&list:rest xs) (&list:first xs) f
                , false
          :examples $ []
            quote $ assert= true
              foldl-compare ([] 1 2 3 4) 0 &<
            quote $ assert= false
              foldl-compare ([] 1 3 2 4) 0 &<
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] (:: 'List 'T) 'T
                :: 'Fn $ {} (:return 'Bool)
                  :args $ [] 'T 'T
              :generics $ [] 'T
        |foldl-shortcut $ %{} :CodeEntry (:doc "|internal function for left fold with shortcut\nSyntax: (foldl-shortcut list initial reducer)\nParams: list (list), initial (any), reducer (function)\nReturns: any\nFolds list from left with early termination support")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic 'Dynamic 'Dynamic 'Fn
          :tags $ #{} :builtin :internal
        |foldr-shortcut $ %{} :CodeEntry (:doc "|internal function for right fold with shortcut\nSyntax: (foldr-shortcut list initial reducer)\nParams: list (list), initial (any), reducer (function)\nReturns: any\nFolds list from right with early termination support")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] (:: 'List 'T) 'Dynamic 'Fn
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |format-cirru $ %{} :CodeEntry (:doc "|internal function for formatting Cirru\nSyntax: (format-cirru data)\nParams: data (list)\nReturns: string\nFormats nested list structure into Cirru syntax text")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'List
          :tags $ #{} :builtin :internal
        |format-cirru-edn $ %{} :CodeEntry (:doc "|internal function for formatting Cirru EDN\nSyntax: (format-cirru-edn data)\nParams: data (any)\nReturns: string\nFormats Calcit data structures into Cirru EDN format text")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |format-cirru-one-liner $ %{} :CodeEntry (:doc "|internal function for formatting Cirru as one-liner\nSyntax: (format-cirru-one-liner data)\nParams: data (list)\nReturns: string\nFormats nested list structure into Cirru one-liner syntax text")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'List
          :tags $ #{} :builtin :internal
        |format-to-cirru $ %{} :CodeEntry (:doc "|internal function for formatting to Cirru syntax\nSyntax: (format-to-cirru value)\nParams: value (any)\nReturns: string in Cirru format\nConverts Calcit data structures to Cirru-style string representation")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |format-to-lisp $ %{} :CodeEntry (:doc "|internal function for formatting to Lisp syntax\nSyntax: (format-to-lisp value)\nParams: value (any)\nReturns: string in Lisp format\nConverts Calcit data structures to Lisp-style string representation")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |frequencies $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn frequencies (xs0)
              assert "|expects a list for frequencies" $ list? xs0
              apply-args
                  {}
                  , xs0
                fn (acc xs)
                  list-match xs
                    () acc
                    (x0 xss)
                      recur
                        if (contains? acc x0)
                          update acc x0 $ \ &+ % 1
                          &map:assoc acc x0 1
                        , xss
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T)
              :generics $ [] 'T
              :return $ :: 'Map 'T 'Number
        |generate-id! $ %{} :CodeEntry (:doc "|internal function for generating unique IDs\nSyntax: (generate-id!)\nParams: none\nReturns: unique string ID\nGenerates a unique identifier string for runtime use")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ []
          :tags $ #{} :builtin :internal :io
        |gensym $ %{} :CodeEntry (:doc "|internal syntax for generating unique symbols\nSyntax: (gensym) or (gensym prefix)\nParams: prefix (string, optional)\nReturns: unique symbol\nGenerates a unique symbol for macro hygiene")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal :syntax
        |get $ %{} :CodeEntry (:doc "|Reads a value from collections or strings by key or index. Handles maps, lists, tuples, records, and strings; nil bases return nil.")
          :code $ quote
            defn get (base k)
              if (nil? base) base $ if (list? base) (&list:nth base k) (.get base k)
          :examples $ []
            quote $ assert= 2
              get ([] 0 2 4) 1
            quote $ assert= |b (get |abc 1)
            quote $ assert= nil (get nil :missing)
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Optional 'T) 'K
              :generics $ [] 'T 'K
              :return $ :: 'Optional 'Dynamic
        |get-char-code $ %{} :CodeEntry (:doc "|internal function for getting character code\nSyntax: (get-char-code char)\nParams: char (string, single character)\nReturns: number\nReturns Unicode code point of character")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'String
          :tags $ #{} :builtin :internal
        |get-env $ %{} :CodeEntry (:doc "|internal function for getting environment variables\nSyntax: (get-env var-name)\nParams: var-name (string)\nReturns: string value or nil\nGets environment variable value, returns nil if not found")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'String
              :return $ :: 'Optional 'String
          :tags $ #{} :builtin :env :internal :io
        |get-in $ %{} :CodeEntry (:doc "|Get value from nested data structure using a path of keys")
          :code $ quote
            defn get-in (base path)
              if
                not $ list? path
                raise $ str-spaced "|expects path in a list, got:" path
              if (nil? base) base $ list-match path
                () base
                (y0 ys)
                  recur (get base y0) ys
          :examples $ []
            quote $ assert= 1
              get-in
                {} $ :a
                  {} $ :b 1
                [] :a :b
            quote $ assert= 2
              get-in
                [] ([] 1 2) ([] 3 4)
                [] 0 1
            quote $ assert= nil
              get-in
                {} $ :x |value
                [] :y
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic (:: 'List 'K)
              :generics $ [] 'K
        |group-by $ %{} :CodeEntry (:doc "|Group elements by the result of applying function f to each element")
          :code $ quote
            defn group-by (xs0 f)
              apply-args
                  {}
                  , xs0
                defn %group-by (acc xs)
                  hint-fn $ {}
                    :args $ []
                      :: 'Map 'K $ :: 'List 'T
                      :: 'List 'T
                    :return $ :: 'Map 'K (:: 'List 'T)
                  list-match xs
                    () acc
                    (x0 xss)
                      let
                          key $ f x0
                        recur
                          if (contains? acc key)
                            update acc key $ \ append % x0
                            &map:assoc acc key $ [] x0
                          , xss
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T)
                :: 'Fn $ {} (:return 'K)
                  :args $ [] 'T
              :generics $ [] 'T 'K
              :return $ :: 'Map 'K (:: 'List 'T)
        |hint-fn $ %{} :CodeEntry (:doc "|internal syntax for function hints (used for async and function schema metadata)\nSyntax: (hint-fn hint-data fn-expr)\nParams: hint-data (schema map or keyword), fn-expr (function)\nReturns: hinted function\nAdds execution hints to functions, including async markers and schema metadata such as :args, :return, :generics, and :where")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :async :builtin :internal :syntax
        |identical? $ %{} :CodeEntry (:doc "|internal function for identity comparison\nSyntax: (identical? a b)\nParams: a (any), b (any)\nReturns: boolean\nReturns true if two values are identical (same reference), not just equal")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T 'U
              :generics $ [] 'T 'U
          :tags $ #{} :builtin :internal
        |identity $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn identity (x) x
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'T)
              :args $ [] 'T
              :generics $ [] 'T
        |if $ %{} :CodeEntry (:doc "|internal syntax for conditional expressions\nSyntax: (if condition then-expr else-expr)\nParams: condition (any), then-expr (any), else-expr (any, optional)\nReturns: value of then-expr if condition is truthy, else-expr otherwise\nEvaluates condition and returns appropriate branch")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ if (> x 0) |positive |non-positive
            quote $ if (empty? xs) 0 (count xs)
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal :syntax
        |if-let $ %{} :CodeEntry (:doc "|Conditionally binds the result of an expression to a symbol and executes the matching branch when the value is non-nil.")
          :code $ quote
            defmacro if-let (pair then ? else)
              if
                not $ and (list? pair)
                  &= 2 $ count pair
                raise $ str-spaced "|expected a pair, got:" pair
              &let
                x $ nth pair 0
                if
                  not $ symbol? x
                  raise $ str-spaced "|expected a symbol for var name, got:" x
                quasiquote $ &let
                  ~x $ ~ (nth pair 1)
                  if (some? ~x) ~then ~else
          :examples $ []
            quote $ assert= |found
              if-let
                v $ :: :some |found
                , v |missing
            quote $ assert= |missing
              if-let
                v $ :: :none
                , v |missing
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic 'Dynamic)
          :tags $ #{} :macro
        |if-not $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro if-not (& xs)
              if
                not $ or
                  &= 2 $ &list:count xs
                  &= 3 $ &list:count xs
                raise $ str-spaced "|if-not expects (condition then) or (condition then else), got:" xs
              &let
                condition $ &list:nth xs 0
                &let
                  true-branch $ &list:nth xs 1
                  &let
                    false-branch $ if
                      &= 3 $ &list:count xs
                      &list:nth xs 2
                      ;nil
                    quasiquote $ if ~condition ~false-branch ~true-branch
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :macro
        |impl-traits $ %{} :CodeEntry (:doc "|Append trait implementations\nSyntax: (impl-traits value & traits)\nParams: value (struct/enum), traits (impl, variadic)\nReturns: value with updated trait implementations\nDispatches to &struct:impl-traits, &enum:impl-traits")
          :code $ quote
            defn impl-traits (x & traits)
              if
                not $ every? traits
                  fn (trait)
                    = :impl $ type-of trait
                raise "|impl-traits misuse. Expected: impl arguments are :impl values. Actual: found non-impl argument. Fix: pass values created by `defimpl`."
              if (struct? x) (&struct:impl-traits x & traits)
                if (enum? x) (&enum:impl-traits x & traits)
                  raise $ str-spaced "|impl-traits misuse. Expected: first argument is struct/enum definition. Actual:" (type-of x) "|Fix: attach impls to `defstruct`/`defenum` result, then construct instances from that definition."
          :examples $ []
          :schema $ :: 'Fn
            {} (:rest 'Impl) (:return 'Dynamic)
              :args $ [] 'Dynamic
        |inc $ %{} :CodeEntry (:doc "|Increments a number by 1")
          :code $ quote
            defn inc (x) (&+ x 1)
          :examples $ []
            quote $ assert= 6 (inc 5)
            quote $ assert= 1 (inc 0)
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number
        |include $ %{} :CodeEntry (:doc "|Add elements to a set, returns a new set with the elements included")
          :code $ quote
            defn include (base & xs)
              reduce xs base $ fn (acc item) (&include acc item)
          :examples $ []
            quote $ assert= (#{} 1 2 3 4)
              include (#{} 1 2) 3 4
            quote $ assert= (#{} 1 2 3)
              include (#{} 1 2) 2 3
          :schema $ :: 'Fn
            {} (:rest 'T)
              :args $ [] (:: 'Set 'T)
              :generics $ [] 'T
              :return $ :: 'Set 'T
        |includes? $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn includes? (x k)
              if (nil? x) false $ if (list? x) (&list:includes? x k) (.includes? x k)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Dynamic 'Dynamic
        |index-of $ %{} :CodeEntry (:doc "|Find the first index of an item in a list, returns nil if not found")
          :code $ quote
            defn index-of (xs item)
              foldl-shortcut xs 0 (;nil)
                defn %index-of (idx x)
                  if (&= item x) (:: true idx)
                    :: false $ &+ 1 idx
          :examples $ []
            quote $ assert= 1
              index-of ([] |a |b |c) |b
            quote $ assert= nil
              index-of ([] 1 2 3) 5
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T) 'T
              :generics $ [] 'T
              :return $ :: 'Optional 'Number
        |interleave $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn interleave (xs0 ys0)
              apply-args
                  []
                  , xs0 ys0
                defn %interleave (acc xs ys)
                  hint-fn $ {}
                    :args $ [] (:: 'List 'Dynamic) (:: 'List 'T) (:: 'List 'U)
                    :generics $ [] 'T 'U
                    :return $ :: 'List 'Dynamic
                  if
                    if (&list:empty? xs) true $ &list:empty? ys
                    , acc $ recur
                      -> acc
                        append $ &list:first xs
                        append $ &list:first ys
                      rest xs
                      rest ys
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'List)
              :args $ [] (:: 'List 'T) (:: 'List 'U)
              :generics $ [] 'T 'U
        |intersection $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn intersection (base & xs)
              reduce xs base $ fn (acc item) (&set:intersection acc item)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Set 'T)
              :generics $ [] 'T
              :rest $ :: 'Set 'T
              :return $ :: 'Set 'T
        |is-spreading-mark? $ %{} :CodeEntry (:doc "|internal function for detecting syntax &\nSyntax: (is-spreading-mark? value)\nParams: value (any)\nReturns: boolean\nReturns true if value is the spreading mark symbol &")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |join $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn join (xs0 sep)
              apply-args
                  []
                  , xs0 true
                defn %join (acc xs beginning?)
                  hint-fn $ {}
                    :args $ [] (:: 'List 'T) (:: 'List 'T) 'Bool
                    :return $ :: 'List 'T
                  list-match xs
                    () acc
                    (x0 xss)
                      recur
                        append
                          if beginning? acc $ append acc sep
                          , x0
                        , xss false
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T) 'T
              :generics $ [] 'T
              :return $ :: 'List 'T
        |join-str $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn join-str (xs0 sep)
              apply-args (| xs0 true)
                defn %join-str (acc xs beginning?)
                  hint-fn $ {}
                    :args $ [] 'String (:: 'List 'T) 'Bool
                    :return 'String
                  list-match xs
                    () acc
                    (x0 xss)
                      recur
                        &str:concat
                          if beginning? acc $ &str:concat acc sep
                          , x0
                        , xss false
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] (:: 'List 'T) 'String
              :generics $ [] 'T
        |js-object $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro js-object (& xs)
              if
                not $ and (list? xs) (every? xs list?)
                raise $ str-spaced "|js-object expects entries in list, got:" xs
              if
                not $ every? xs
                  fn (entry)
                    &= 2 $ &list:count entry
                raise $ str-spaced "|js-object expects each entry as pair, got:" xs
              &let
                ys $ &list:concat & xs
                quasiquote $ &js-object ~@ys
          :examples $ []
          :schema $ :: 'Macro
            {}
              :args $ [] 'Dynamic
              :features $ #{} :js-ffi
          :tags $ #{} :interop :macro
        |json-parse $ %{} :CodeEntry (:doc "|internal function for parsing JSON text\nSyntax: (json-parse text)\nParams: text (string)\nReturns: Calcit data\nParses JSON text into Calcit values. JSON object keys become tags, arrays become lists, and null becomes nil")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ assert=
              {} $ :a 1
              json-parse "|{\"a\":1}"
            quote $ assert= ([] true nil)
              get (json-parse "|{\"items\":[true,null]}") :items
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'String
          :tags $ #{} :builtin :internal
        |json-pretty $ %{} :CodeEntry (:doc "|internal function for pretty-printing JSON\nSyntax: (json-pretty value)\nParams: value (any JSON-compatible Calcit data)\nReturns: string\nConverts Calcit data into formatted JSON text using 2-space indentation")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ assert= "|{\n  \"a\": 1\n}"
              json-pretty $ {} (:a 1)
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |json-stringify $ %{} :CodeEntry (:doc "|internal function for encoding JSON\nSyntax: (json-stringify value)\nParams: value (any JSON-compatible Calcit data)\nReturns: string\nConverts Calcit data into compact JSON text. Tags and symbols are encoded as plain JSON strings")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ assert= "|\"ok\"" (json-stringify :ok)
            quote $ assert= "|{\"a\":1}"
              json-stringify $ {} (:a 1)
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |keys $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn keys (x)
              map (to-pairs x) &list:first
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Set)
              :args $ [] 'Dynamic
        |keys-non-nil $ %{} :CodeEntry (:doc "|Get keys from a map that have non-nil values")
          :code $ quote
            defn keys-non-nil (x)
              apply-args
                  #{}
                  to-pairs x
                fn (acc pairs)
                  hint-fn $ {}
                    :args $ [] (:: 'Set 'K) 'Set
                    :return $ :: 'Set 'K
                  if (&set:empty? pairs) acc $ &let
                    set-pair $ &set:destruct pairs
                    &let
                      pair $ nth set-pair 0
                      if
                        nil? $ last pair
                        recur acc $ nth set-pair 1
                        recur
                          include acc $ &list:first pair
                          nth set-pair 1
          :examples $ []
            quote $ assert= (#{} :a :b)
              keys-non-nil $ {} (:a 1) (:b 2) (:c nil)
            quote $ assert= (#{})
              keys-non-nil $ {} (:a nil) (:b nil)
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V)
              :generics $ [] 'K 'V
              :return $ :: 'Set 'K
        |last $ %{} :CodeEntry (:doc "|Returns the last element of a list-like collection\nReturns nil when the collection is empty.")
          :code $ quote
            defn last (xs)
              if (empty? xs) (;nil)
                nth xs $ &- (count xs) 1
          :examples $ []
            quote $ assert= 3
              last $ [] 1 2 3
            quote $ assert= nil
              last $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Optional 'T)
              :generics $ [] 'T
              :return $ :: 'Optional 'Dynamic
        |let $ %{} :CodeEntry (:doc "|macro for local bindings\nSyntax: (let ([name value] ...) body...)\nParams: pairs (list of binding pairs), body (expressions)\nReturns: result of body with bindings in scope\nCreates multiple local bindings sequentially")
          :code $ quote
            defmacro let (pairs & body)
              if
                not $ and (list? pairs) (every? pairs list?)
                raise $ str-spaced "|expects pairs in list for let, got:" pairs
              if
                &= 1 $ &list:count pairs
                quasiquote $ &let
                  ~ $ &list:nth pairs 0
                  ~@ body
                if (&list:empty? pairs)
                  quasiquote $ &let () ~@body
                  quasiquote $ &let
                    ~ $ &list:nth pairs 0
                    let
                      ~ $ &list:rest pairs
                      ~@ body
          :examples $ []
            quote $ let
                x 1
                y 2
              + x y
            quote $ let
                a 10
              * a a
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |let-destruct $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro let-destruct (pattern v & body)
              if (symbol? pattern)
                quasiquote $ &let (~pattern ~v) ~@body
                if (list? pattern)
                  if
                    &= [] $ &list:first pattern
                    quasiquote $ let[]
                      ~ $ &list:rest pattern
                      , ~v ~@body
                    if
                      &= '{} $ &list:first pattern
                      quasiquote $ let{}
                        ~ $ &list:rest pattern
                        , ~v ~@body
                      raise $ str-spaced "|Unknown pattern to destruct:" pattern
                  raise $ str-spaced "|Unknown structure to destruct:" pattern
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic 'Dynamic)
          :tags $ #{} :macro
        |let-sugar $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro let-sugar (pairs & body)
              if
                not $ and (list? pairs) (every? pairs list?)
                raise $ str-spaced "|expects pairs in list for let, got:" pairs
              if (&list:empty? pairs)
                quasiquote $ &let () ~@body
                &let
                  pair $ &list:first pairs
                  if
                    not $ &= 2 (&list:count pair)
                    raise $ str-spaced "|expected pair length of 2, got:" pair
                  if
                    &= 1 $ &list:count pairs
                    quasiquote $ let-destruct ~@pair (~@ body)
                    quasiquote $ let-destruct ~@pair
                      let-sugar
                        ~ $ &list:rest pairs
                        ~@ body
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |let[] $ %{} :CodeEntry (:doc "|Destructures a sequential value inside `let`, assigning each position to declared names and supporting `&` rest bindings.")
          :code $ quote
            defmacro let[] (vars data & body)
              if
                not $ and (list? vars)
                  every? vars $ fn (x)
                    or (symbol? x) (is-spreading-mark? x)
                raise $ str-spaced "|expects a list of definitions, got:" vars
              let
                  variable? $ symbol? data
                  v $ if variable? data (gensym |v)
                  defs $ apply-args
                    [] ([]) vars 0
                    defn let[]% (acc xs idx)
                      if (&list:empty? xs) acc $ &let ()
                        if
                          not $ or
                            symbol? $ &list:first xs
                            is-spreading-mark? $ &list:first xs
                          raise $ &str:concat "|Expected symbol for vars: " (&list:first xs)
                        if
                          is-spreading-mark? $ &list:first xs
                          &let ()
                            assert "|expected list spreading" $ &= 2 (&list:count xs)
                            append acc $ [] (&list:nth xs 1)
                              quasiquote $ &list:slice ~v ~idx
                          recur
                            append acc $ [] (&list:first xs)
                              quasiquote $ &list:nth ~v ~idx
                            rest xs
                            inc idx
                if variable?
                  quasiquote $ let (~ defs) (~@ body)
                  quasiquote $ &let (~v ~data)
                    let (~ defs) (~@ body)
          :examples $ []
            quote $ let[] (x y) ([] 1 2) (+ x y)
            quote $ let[] (head & tail) ([] 9 8 7) (count tail)
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic 'Dynamic)
          :tags $ #{} :macro
        |let{} $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro let{} (items base & body)
              if
                not $ and (list? items) (every? items symbol?)
                raise $ str-spaced "|expects symbol names in binding names, got:" items
              &let
                var-result $ gensym |result
                quasiquote $ &let (~var-result ~base)
                  assert (str "|expected map for destructing: " ~var-result) (map? ~var-result)
                  let
                    ~ $ map items
                      defn gen-items% (x)
                        [] x $ [] (turn-tag x) var-result
                    ~@ body
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic 'Dynamic)
          :tags $ #{} :macro
        |list-match $ %{} :CodeEntry (:doc "|Two-branch list destructuring macro. Provides separate clauses for the empty list and a head/tail pattern, useful for simple recursion or guards.")
          :code $ quote
            defmacro list-match (& values)
              if
                not $ &= 3 (&list:count values)
                raise $ str-spaced "|list-match expects exactly 3 arguments, got:" values
              &let
                xs $ &list:nth values 0
                &let
                  pattern1 $ &list:nth values 1
                  &let
                    pattern2 $ &list:nth values 2
                    if
                      not $ and (list? pattern1) (list? pattern2)
                        &> (count pattern1) 1
                        list? $ &list:nth pattern1 0
                        list? $ &list:nth pattern2 0
                        &> (count pattern2) 1
                      raise $ str-spaced "|list-match expects 2 branches: (() body...) and ((head tail) body...), got:" pattern1 pattern2
                    &let
                      v# $ gensym |v
                      quasiquote $ &let (~v# ~xs)
                        if
                          not $ list? ~v#
                          raise "|expected a list in list-match"
                        ~ $ if
                          and
                            empty? $ &list:nth pattern1 0
                            &= 2 $ count (&list:nth pattern2 0)
                          quasiquote $ &list-match-internal ~v#
                            ~ $ &list:slice pattern1 1
                            ~ $ &list:nth pattern2 0
                            ~ $ &list:slice pattern2 1
                          if
                            and
                              empty? $ &list:nth pattern2 0
                              &= 2 $ count (&list:nth pattern1 0)
                            quasiquote $ &list-match-internal ~v#
                              ~ $ &list:slice pattern2 1
                              ~ $ &list:nth pattern1 0
                              ~ $ &list:slice pattern1 1
                            raise $ str-spaced "|list-match expects one empty branch and one destructuring branch, got:" pattern1 pattern2
          :examples $ []
            quote $ assert= :something
              list-match ([] 1)
                () :empty
                (a b) :something
            quote $ assert= 1
              list-match ([] 1 2 3)
                () nil
                (head tail) head
          :schema $ :: 'Dynamic
          :tags $ #{} :macro
        |list? $ %{} :CodeEntry (:doc "|checks if value is a list\nSyntax: (list? x)\nParams: x (any)\nReturns: true if x is a list, false otherwise\nType predicate for list data structure")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ list? ([] 1 2 3)
            quote $ list? ({})
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |loop $ %{} :CodeEntry (:doc "|Named-let style looping macro. Binds initial values once and uses `recur` to update bindings in a tail-recursive way without stack growth.")
          :code $ quote
            defmacro loop (pairs & body)
              if
                not $ list? pairs
                raise $ str-spaced "|expects pairs in loop, got:" pairs
              if
                not $ every? pairs
                  defn detect-pairs? (x)
                    if (list? x)
                      &= 2 $ &list:count x
                      , false
                raise $ str-spaced "|expects pairs in pairs in loop, got:" pairs
              let
                  args $ map pairs &list:first
                  values $ map pairs last
                assert "|loop requires symbols in pairs" $ every? args symbol?
                quasiquote $ apply (defn generated-loop ~args ~@body) ([] ~@values)
          :examples $ []
            quote $ assert= 6
              loop
                  total 0
                (xs ([] 1 2 3))
                if (empty? xs) total $ recur
                  + total $ first xs
                  rest xs
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |macro? $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn macro? (x)
              &= (type-of x) :macro
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T
              :generics $ [] 'T
        |macroexpand $ %{} :CodeEntry (:doc "|internal syntax for expanding macros until recursive calls are resolved\nSyntax: (macroexpand expr)\nParams: expr (macro call)\nReturns: fully expanded code\nExpands macros recursively until no more macro calls remain")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal :meta :syntax
        |macroexpand-1 $ %{} :CodeEntry (:doc "|internal syntax for expanding macro just once for debugging\nSyntax: (macroexpand-1 expr)\nParams: expr (macro call)\nReturns: one-level expanded code\nExpands macro only one level for debugging purposes")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal :meta :syntax
        |macroexpand-all $ %{} :CodeEntry (:doc "|internal syntax for expanding macro until macros inside are resolved\nSyntax: (macroexpand-all expr)\nParams: expr (code with macros)\nReturns: fully expanded code\nExpands all macros including nested ones")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal :meta :syntax
        |map $ %{} :CodeEntry (:doc "|Collection mapping function. Applies a function to each element of a list, set, or map, returning a structure of the same shape.")
          :code $ quote
            defn map (xs f)
              if (list? xs) (&list:map xs f)
                if (set? xs)
                  foldl xs (#{})
                    defn %map (acc x)
                      hint-fn $ {}
                        :args $ [] 'Set 'Dynamic
                        :return 'Set
                      include acc $ f x
                  .map xs f
          :examples $ []
            quote $ assert= ([] 2 3 4)
              map ([] 1 2 3) inc
            quote $ assert= ([] |1 |2 |3)
              map ([] 1 2 3) str
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'C
                :: 'Fn $ {} (:return 'U)
                  :args $ [] 'T
              :generics $ [] 'C 'T 'U
              :where $ {} ('C 'Mappable)
        |map-indexed $ %{} :CodeEntry (:doc "|Map over a collection with index, f takes index and value")
          :code $ quote
            defn map-indexed (xs f)
              foldl xs ([])
                defn %map-indexed (acc x)
                  hint-fn $ {}
                    :generics $ [] 'U
                    :args $ []
                      :: 'acc $ :: 'List 'U
                      :: 'x 'Dynamic
                    :return $ :: 'List 'U
                  append acc $ f (count acc) x
          :examples $ []
            quote $ assert= ([] 10 21 32)
              map-indexed ([] 10 20 30)
                fn (i x) (&+ i x)
            quote $ assert=
              [] ([] 0 |a) ([] 1 |b)
              map-indexed ([] |a |b)
                fn (i x) ([] i x)
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic 'Fn
        |map-kv $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn map-kv (xs f)
              foldl xs ({})
                defn %map-kv (acc pair)
                  hint-fn $ {}
                    :args $ [] 'Map 'List
                    :return 'Map
                  &let
                    result $ f (nth pair 0) (nth pair 1)
                    if (list? result)
                      do
                        assert "|expected pair returned when mapping hashmap" $ &= 2 (&list:count result)
                        &map:assoc acc (nth result 0) (nth result 1)
                      if
                        or (nil? result) (tuple? result)
                        , acc $ raise (str-spaced "|map-kv expected list or nil, got:" result)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V)
                :: 'Fn $ {} (:return 'Q)
                  :args $ [] 'K 'V
              :generics $ [] 'K 'V 'Q 'R 'S
              :return $ :: 'Map 'R 'S
        |map? $ %{} :CodeEntry (:doc "|Predicate that checks whether a value is a map")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ assert= true
              map? $ {} (:a 1)
            quote $ assert= false
              map? $ [] 1 2
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |mapcat $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn mapcat (xs f)
              &list:concat & $ map xs f
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T)
                :: 'Fn $ {}
                  :args $ [] 'T
                  :return $ :: 'List 'U
              :generics $ [] 'T 'U
              :return $ :: 'List 'U
        |max $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn max (xs) (.max xs)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'T
              :generics $ [] 'T
              :return $ :: 'Optional 'T
        |merge $ %{} :CodeEntry (:doc "|Combines maps left-to-right, with later maps overwriting keys from earlier ones by reducing through `&merge`.")
          :code $ quote
            defn merge (x0 & xs) (reduce xs x0 &merge)
          :examples $ []
            quote $ assert=
              {} (:a 2) (:b 1)
              merge
                {} $ :a 1
                {} (:a 2) (:b 1)
            quote $ assert=
              {} (:a 1) (:b 2) (:c 3)
              merge
                {} $ :a 1
                {} $ :b 2
                {} $ :c 3
          :schema $ :: 'Fn
            {} (:rest 'Dynamic) (:return 'T)
              :args $ [] 'T
              :generics $ [] 'T
        |merge-non-nil $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn merge-non-nil (x0 & xs) (reduce xs x0 &merge-non-nil)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V)
              :generics $ [] 'K 'V
              :rest $ :: 'Map 'K 'V
              :return $ :: 'Map 'K 'V
        |min $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn min (xs) (.min xs)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'T
              :generics $ [] 'T
              :return $ :: 'Optional 'T
        |negate $ %{} :CodeEntry (:doc "|Negate a number, returns its opposite")
          :code $ quote
            defn negate (x) (&- 0 x)
          :examples $ []
            quote $ assert= -5 (negate 5)
            quote $ assert= 3 (negate -3)
            quote $ assert= 0 (negate 0)
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number
        |nil? $ %{} :CodeEntry (:doc "|Predicate that checks whether a value is nil")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ assert= true (nil? nil)
            quote $ assert= false (nil? 0)
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |non-nil! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn non-nil! (x)
              if (nil? x) (raise "|expected non nil value") x
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'T)
              :args $ [] 'T
              :generics $ [] 'T
        |not $ %{} :CodeEntry (:doc "|internal function for logical not\nSyntax: (not value)\nParams: value (any)\nReturns: boolean\nReturns true if value is falsy (nil or false), false otherwise")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |not= $ %{} :CodeEntry (:doc "|Returns true when its two arguments are not identical according to `=`.")
          :code $ quote
            defn not= (x y)
              not $ &= x y
          :examples $ []
            quote $ assert= true (not= 1 2)
            quote $ assert= false (not= :a :a)
            quote $ assert= true (not= |a |b)
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Dynamic 'Dynamic
        |noted $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro noted (_doc v) v
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic 'Dynamic)
          :tags $ #{} :macro
        |nth $ %{} :CodeEntry (:doc "|Returns the element at index `i` from a list, tuple, or sequential data structure\nRaises if the index is outside the available range.")
          :code $ quote
            defn nth (x i)
              if (list? x) (&list:nth x i) (.nth x i)
          :examples $ []
            quote $ assert= 2
              nth ([] 1 2 3) 1
            quote $ assert= |b (nth |abc 1)
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic 'Number
        |number? $ %{} :CodeEntry (:doc "|Predicate that checks whether a value is a numeric scalar")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ assert= true (number? 123)
            quote $ assert= true (number? 3.14)
            quote $ assert= false (number? |text)
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |option:and-then $ %{} :CodeEntry (:doc "|Chains an Option-producing function over :some and preserves :none.")
          :code $ quote
            defn option:and-then (opt f)
              tag-match opt
                (:some value) (f value)
                (:none)
                  %:: (&tuple:enum opt) :none
          :examples $ []
            quote $ assert= (%some 4)
              option:and-then (%some 2)
                fn (x)
                  %some $ * x 2
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Option 'T)
                :: 'Fn $ {}
                  :args $ [] 'T
                  :return $ :: 'Option 'U
              :generics $ [] 'T 'U
              :return $ :: 'Option 'U
        |option:map $ %{} :CodeEntry (:doc "|Mappable map implementation for Option")
          :code $ quote
            defn option:map (opt f)
              tag-match opt
                (:some value)
                  %:: (&tuple:enum opt) :some $ f value
                (:none)
                  %:: (&tuple:enum opt) :none
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Option 'T)
                :: 'Fn $ {} (:return 'U)
                  :args $ [] 'T
              :generics $ [] 'T 'U
              :return $ :: 'Option 'U
        |option:none? $ %{} :CodeEntry (:doc "|Returns true when an Option is :none.")
          :code $ quote
            defn option:none? (opt)
              tag-match opt
                (:some _) false
                (:none) true
          :examples $ []
            quote $ assert= true
              option:none? $ %none
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] (:: 'Option 'T)
              :generics $ [] 'T
        |option:some? $ %{} :CodeEntry (:doc "|Returns true when an Option is :some.")
          :code $ quote
            defn option:some? (opt)
              tag-match opt
                (:some _) true
                (:none) false
          :examples $ []
            quote $ assert= true
              option:some? $ %some 1
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] (:: 'Option 'T)
              :generics $ [] 'T
        |option:unwrap-or $ %{} :CodeEntry (:doc "|Returns the :some payload, or the fallback for :none.")
          :code $ quote
            defn option:unwrap-or (opt fallback)
              tag-match opt
                (:some value) value
                (:none) fallback
          :examples $ []
            quote $ assert= 0
              (%none) .unwrap-or 0
          :schema $ :: 'Fn
            {} (:return 'T)
              :args $ [] (:: 'Option 'T) 'T
              :generics $ [] 'T
        |optionally $ %{} :CodeEntry (:doc "|Convert a nullable Optional<T> value into nominal Option<T>.")
          :code $ quote
            defn optionally (s)
              if (nil? s) (:: :none) (:: :some s)
          :examples $ []
            quote $ assert= (%some 1) (optionally 1)
            quote $ assert= (%none) (optionally nil)
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Optional 'T)
              :generics $ [] 'T
              :return $ :: 'Option 'T
        |or $ %{} :CodeEntry (:doc "|Logical disjunction macro. Skips evaluating later forms once a truthy (non-nil, non-false) value is found, preserving the first truthy result.")
          :code $ quote
            defmacro or (item & xs)
              if (&list:empty? xs) item $ if (list? item)
                &let
                  v1# $ gensym |v1
                  quasiquote $ &let (~v1# ~item)
                    if
                      if (nil? ~v1#) true $ &= false ~v1#
                      or
                        ~ $ &list:first xs
                        ~@ $ &list:rest xs
                      ~ v1#
                quasiquote $ if
                  if (nil? ~item) true $ &= false ~item
                  or
                    ~ $ &list:first xs
                    ~@ $ &list:rest xs
                  ~ item
          :examples $ []
            quote $ assert= |done (or nil |done false)
            quote $ assert= false (or false nil)
            quote $ assert= 2 (or nil 2 3)
          :schema $ :: 'Dynamic
          :tags $ #{} :macro
        |pairs-map $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn pairs-map (xs)
              reduce xs ({})
                defn %pairs-map (acc pair)
                  hint-fn $ {}
                    :args $ [] 'Map 'List
                    :return 'Map
                  assert "|expects pair for pairs-map" $ if (list? pair)
                    &= 2 $ &list:count pair
                    , false
                  &map:assoc acc (&list:first pair) (last pair)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'P)
              :generics $ [] 'P 'K 'V
              :return $ :: 'Map 'K 'V
        |parse-cirru $ %{} :CodeEntry (:doc "|internal function for parsing Cirru\nSyntax: (parse-cirru text)\nParams: text (string)\nReturns: list\nParses Cirru syntax text into nested list structure")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'CirruQuote)
              :args $ [] 'String
          :tags $ #{} :builtin :internal
        |parse-cirru-edn $ %{} :CodeEntry (:doc "|internal function for parsing Cirru EDN\nSyntax: (parse-cirru-edn text)\nParams: text (string)\nReturns: any\nParses Cirru EDN format text into Calcit data structures")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'String
          :tags $ #{} :builtin :internal
        |parse-cirru-list $ %{} :CodeEntry (:doc "|internal function for parsing Cirru list\nSyntax: (parse-cirru-list text)\nParams: text (string)\nReturns: list\nParses Cirru text as a list of expressions")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'List)
              :args $ [] 'String
          :tags $ #{} :builtin :internal
        |parse-float $ %{} :CodeEntry (:doc "|internal function for parsing float\nSyntax: (parse-float s)\nParams: s (string)\nReturns: number or nil\nParses string as floating point number, returns nil if invalid")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'String
              :return $ :: 'Optional 'Number
          :tags $ #{} :builtin :internal
        |pow $ %{} :CodeEntry (:doc "|internal function for power operation\nSyntax: (pow base exponent)\nParams: base (number), exponent (number)\nReturns: number\nRaises base to the power of exponent")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number 'Number
          :tags $ #{} :builtin :internal
        |prepend $ %{} :CodeEntry (:doc "|internal function for prepending to list\nSyntax: (prepend list element)\nParams: list (list), element (any)\nReturns: list\nReturns new list with element added at beginning")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T) 'T
              :generics $ [] 'T
              :return $ :: 'List 'T
          :tags $ #{} :builtin :internal
        |quasiquote $ %{} :CodeEntry (:doc "|internal syntax for quasiquote (used inside macros)\nSyntax: (quasiquote expr)\nParams: expr (code with possible unquote)\nReturns: partially quoted structure\nLike quote but allows selective unquoting with ~ and ~@")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ quasiquote (&+ ~x 1)
            quote $ quasiquote ([] ~x ~@xs)
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal :syntax
        |quit! $ %{} :CodeEntry (:doc "|internal function for quitting program\nSyntax: (quit! exit-code)\nParams: exit-code (number, optional, defaults to 0)\nReturns: never returns (exits program)\nTerminates the program with specified exit code")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ [] (:: 'Optional 'Number)
          :tags $ #{} :builtin :control :internal
        |quote $ %{} :CodeEntry (:doc "|internal syntax for turning code into quoted data\nSyntax: (quote expr)\nParams: expr (any code)\nReturns: quoted data structure\nPrevents evaluation and returns code as data")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal :syntax
        |raise $ %{} :CodeEntry (:doc "|internal function for raising exceptions\nSyntax: (raise message)\nParams: message (string)\nReturns: never returns (throws exception)\nThrows an exception with the given message")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'T)
              :args $ [] 'String
              :generics $ [] 'T
          :tags $ #{} :builtin :control :internal
        |range $ %{} :CodeEntry (:doc "|internal function for creating number ranges\nSyntax: (range start end) or (range end)\nParams: start (number, optional), end (number)\nReturns: list\nCreates list of numbers from start to end (exclusive)")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'Number (:: 'Optional 'Number)
              :return $ :: 'List 'Number
          :tags $ #{} :builtin :internal
        |range-bothway $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn range-bothway (x ? y)
              if-let (y0 y)
                range
                  inc $ &- (&+ x x) y0
                  , y0
                range
                  inc $ negate x
                  , x
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'Number (:: 'Optional 'Number)
              :return $ :: 'List 'Number
        |read-dir $ %{} :CodeEntry (:doc "|List paths inside a directory.\nSyntax: (read-dir path recursive?)\nParams: path (string), recursive? (optional boolean, defaults to false)\nReturns: sorted list of path strings")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'String (:: 'Optional 'Bool)
              :return $ :: 'List 'String
          :tags $ #{} :builtin :file :internal :io
        |read-file $ %{} :CodeEntry (:doc "|internal function for reading files\nSyntax: (read-file filepath)\nParams: filepath (string)\nReturns: string content or error\nReads file content as string, throws error if file not found")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'String
          :tags $ #{} :builtin :file :internal :io
        |record-match $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro record-match (value & body)
              if (&list:empty? body) (raise "|record-match expected patterns for matching")
                if (list? value)
                  &let
                    v# $ gensym |v
                    quasiquote $ &let (~v# ~value)
                      assert "|expected record to match" $ record? ~v#
                      &record-match-internal ~v# ~@body
                  quasiquote $ &let ()
                    assert "|expected record to match" $ record? ~value
                    &record-match-internal ~value ~@body
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Record)
          :tags $ #{} :macro
        |record-with $ %{} :CodeEntry (:doc "|macro to extend existing record with new values in pairs, internally using &record:with which takes flattern items")
          :code $ quote
            defmacro record-with (record & pairs) (; "check if args are in pairs")
              if
                not $ and (list? pairs)
                  every? pairs $ fn (xs)
                    and (list? xs)
                      &= 2 $ count xs
                raise $ str-spaced "|record-with expects a list of pairs (each with exactly two elements), got:" pairs
              ; "call &record:with"
              quasiquote $ &record:with ~record
                ~@ $ &list:concat & pairs
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |record? $ %{} :CodeEntry (:doc "|Predicate that checks whether a value is a struct-based record (created with defstruct + %{}).")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ assert= false
              record? $ {} (:x 1)
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |recur $ %{} :CodeEntry (:doc "|internal function for tail recursion\nSyntax: (recur args...)\nParams: args (any, variable number)\nReturns: recur structure for tail call optimization\nEnables tail call optimization by marking recursive calls")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal
        |reduce $ %{} :CodeEntry (:doc "|Collection reduction operation\nFunction: Reduces a collection using a specified function, accumulating elements onto an initial value\nParams: xs (collection), x0 (initial accumulator value), f (reduction function that takes accumulator and current element)\nReturns: any type - final accumulated result\nNotes: The reduction function f should accept two parameters (accumulator, current element) and return a new accumulator value")
          :code $ quote
            defn reduce (xs x0 f) (foldl xs x0 f)
          :examples $ []
            quote $ assert= 6
              reduce ([] 1 2 3) 0 +
          :schema $ :: 'Fn
            {} (:return 'U)
              :args $ [] (:: 'List 'T) 'U
                :: 'Fn $ {} (:return 'U)
                  :args $ [] 'U 'T
              :generics $ [] 'T 'U
        |ref? $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn ref? (x)
              &= (type-of x) :ref
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal :state
        |remove-watch $ %{} :CodeEntry (:doc "|internal function for removing atom watchers\nSyntax: (remove-watch atom key)\nParams: atom (atom), key (any)\nReturns: atom\nRemoves watcher with specified key from atom")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ [] (:: 'Ref 'T) 'K
              :generics $ [] 'T 'K
          :tags $ #{} :builtin :internal :state :watch
        |repeat $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn repeat (x n0)
              apply-args
                  []
                  , n0
                defn %repeat (acc n)
                  hint-fn $ {}
                    :generics $ [] 'T
                    :args $ []
                      :: 'acc $ :: 'List 'T
                      :: 'n 'Number
                    :return $ :: 'List 'T
                  if (&<= n 0) acc $ recur (append acc x) (&- n 1)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'T 'Number
              :generics $ [] 'T
              :return $ :: 'List 'T
        |reset! $ %{} :CodeEntry (:doc "|internal syntax for resetting atom values\nSyntax: (reset! atom new-value)\nParams: atom (atom reference), new-value (any)\nReturns: new value\nSets atom to new value and returns it")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ ; reset! *my-atom
              {} $ :a 2
          :schema $ :: 'Fn
            {} (:return 'T)
              :args $ [] (:: 'Ref 'T) 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal :state :syntax
        |rest $ %{} :CodeEntry (:doc "|Returns the collection without its first element\nNil input returns nil; lists delegate to &list:rest.")
          :code $ quote
            defn rest (x)
              if (nil? x) (;nil)
                if (list? x) (&list:rest x) (.rest x)
          :examples $ []
            quote $ assert= ([] 2 3)
              rest $ [] 1 2 3
            quote $ assert= nil (rest nil)
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Optional 'T)
              :generics $ [] 'T
              :return $ :: 'Optional 'T
        |result:and-then $ %{} :CodeEntry (:doc "|Chains a Result-producing function over :ok and preserves :err.")
          :code $ quote
            defn result:and-then (res f)
              tag-match res
                (:ok value) (f value)
                (:err err)
                  %:: (&tuple:enum res) :err err
          :examples $ []
            quote $ assert= (%ok 4)
              result:and-then (%ok 2)
                fn (x)
                  %ok $ * x 2
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Result 'T 'E)
                :: 'Fn $ {}
                  :args $ [] 'T
                  :return $ :: 'Result 'U 'E
              :generics $ [] 'T 'U 'E
              :return $ :: 'Result 'U 'E
        |result:err? $ %{} :CodeEntry (:doc "|Returns true when a Result is :err.")
          :code $ quote
            defn result:err? (res)
              tag-match res
                (:ok _) false
                (:err _) true
          :examples $ []
            quote $ assert= true
              result:err? $ %err |failed
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] (:: 'Result 'T 'E)
              :generics $ [] 'T 'E
        |result:map $ %{} :CodeEntry (:doc "|Mappable map implementation for Result")
          :code $ quote
            defn result:map (res f)
              tag-match res
                (:ok value)
                  %:: (&tuple:enum res) :ok $ f value
                (:err err)
                  %:: (&tuple:enum res) :err err
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Result 'T 'E)
                :: 'Fn $ {} (:return 'U)
                  :args $ [] 'T
              :generics $ [] 'T 'U 'E
              :return $ :: 'Result 'U 'E
        |result:map-err $ %{} :CodeEntry (:doc "|Maps the error payload of a Result while preserving :ok.")
          :code $ quote
            defn result:map-err (res f)
              tag-match res
                (:ok value)
                  %:: (&tuple:enum res) :ok value
                (:err err)
                  %:: (&tuple:enum res) :err $ f err
          :examples $ []
            quote $ assert= (%err |failed!)
              result:map-err (%err |failed)
                fn (e) (str e |!)
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Result 'T 'E)
                :: 'Fn $ {} (:return 'F)
                  :args $ [] 'E
              :generics $ [] 'T 'E 'F
              :return $ :: 'Result 'T 'F
        |result:ok? $ %{} :CodeEntry (:doc "|Returns true when a Result is :ok.")
          :code $ quote
            defn result:ok? (res)
              tag-match res
                (:ok _) true
                (:err _) false
          :examples $ []
            quote $ assert= true
              result:ok? $ %ok 1
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] (:: 'Result 'T 'E)
              :generics $ [] 'T 'E
        |result:unwrap-or $ %{} :CodeEntry (:doc "|Returns the :ok payload, or the fallback for :err.")
          :code $ quote
            defn result:unwrap-or (res fallback)
              tag-match res
                (:ok value) value
                (:err _) fallback
          :examples $ []
            quote $ assert= 0
              (%err |failed) .unwrap-or 0
          :schema $ :: 'Fn
            {} (:return 'T)
              :args $ [] (:: 'Result 'T 'E) 'T
              :generics $ [] 'T 'E
        |reverse $ %{} :CodeEntry (:doc "|Reverse the order of elements in a list")
          :code $ quote
            defn reverse (x) (&list:reverse x)
          :examples $ []
            quote $ assert= ([] 3 2 1)
              reverse $ [] 1 2 3
            quote $ assert= ([])
              reverse $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T)
              :generics $ [] 'T
              :return $ :: 'List 'T
        |round $ %{} :CodeEntry (:doc "|internal function for rounding numbers\nSyntax: (round n)\nParams: n (number)\nReturns: number\nRounds number to nearest integer")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number
          :tags $ #{} :builtin :internal
        |round? $ %{} :CodeEntry (:doc "|internal function for checking if number is round\nSyntax: (round? n)\nParams: n (number)\nReturns: boolean\nReturns true if number has no fractional part")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Number
          :tags $ #{} :builtin :internal
        |section-by $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn section-by (xs0 n)
              if (>= n 1)
                apply-args
                    []
                    , xs0
                  fn (acc xs)
                    if
                      &<= (&list:count xs) n
                      if (&list:empty? xs) acc $ append acc xs
                      recur
                        append acc $ take xs n
                        drop xs n
                raise $ str-spaced "|expected positive number, got:" n
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T) 'Number
              :generics $ [] 'T
              :return $ :: 'List (:: 'List 'T)
        |select-keys $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn select-keys (m xs)
              assert "|expected map for selecting" $ map? m
              foldl xs (&{})
                defn %select-keys (acc k)
                  hint-fn $ {}
                    :args $ [] 'Map 'Dynamic
                    :return 'Map
                  &map:assoc acc k $ &map:get m k
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V) (:: 'List 'K)
              :generics $ [] 'K 'V
              :return $ :: 'Map 'K 'V
        |set? $ %{} :CodeEntry (:doc "|Check if a value is a set")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ assert= true
              set? $ #{} 1 2 3
            quote $ assert= false
              set? $ [] 1 2 3
            quote $ assert= false
              set? $ {} (:a 1)
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |sin $ %{} :CodeEntry (:doc "|internal function for sine\nSyntax: (sin n)\nParams: n (number, radians)\nReturns: number\nReturns sine of angle in radians")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number
          :tags $ #{} :builtin :internal
        |slice $ %{} :CodeEntry (:doc "|Extract a slice from a collection from index n to m")
          :code $ quote
            defn slice (xs n ? m)
              if (nil? xs) (;nil)
                if (list? xs) (&list:slice xs n m)
                  if (string? xs) (&str:slice xs n m) (.slice xs n m)
          :examples $ []
            quote $ assert= ([] 2 3)
              slice ([] 1 2 3 4) 1 3
            quote $ assert= ([] 3 4)
              slice ([] 1 2 3 4) 2
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic 'Number (:: 'Optional 'Number)
        |some-in? $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn some-in? (x path)
              if (nil? x) false $ list-match path
                () true
                (k ps)
                  if (map? x)
                    if (contains? x k)
                      recur (get x k) ps
                      , false
                    if (list? x)
                      if (number? k)
                        recur (get x k) ps
                        , false
                      raise $ &str:concat "|Unknown structure for some-in? detection: " x
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Dynamic (:: 'List 'K)
              :generics $ [] 'K
        |some? $ %{} :CodeEntry (:doc "|Complement of nil?\nReturns true when the value is not nil.")
          :code $ quote
            defn some? (x)
              not $ nil? x
          :examples $ []
            quote $ assert= true (some? 0)
            quote $ assert= false (some? nil)
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Dynamic
        |sort $ %{} :CodeEntry (:doc "|internal function for sorting lists\nSyntax: (sort list) or (sort list comparator)\nParams: list (list), comparator (function, optional)\nReturns: list\nReturns sorted list using natural order or custom comparator")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T) (:: 'Optional 'Fn)
              :generics $ [] 'T
              :return $ :: 'List 'T
          :tags $ #{} :builtin :internal
        |split $ %{} :CodeEntry (:doc "|internal function for splitting strings\nSyntax: (split s delimiter)\nParams: s (string), delimiter (string)\nReturns: list of strings\nSplits string by delimiter into list of substrings")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'String 'String
              :return $ :: 'List 'String
          :tags $ #{} :builtin :internal
        |split-lines $ %{} :CodeEntry (:doc "|internal function for splitting lines\nSyntax: (split-lines s)\nParams: s (string)\nReturns: list of strings\nSplits string by newlines into list of lines")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'String
              :return $ :: 'List 'String
          :tags $ #{} :builtin :internal
        |sqrt $ %{} :CodeEntry (:doc "|internal function for square root\nSyntax: (sqrt n)\nParams: n (number)\nReturns: number\nReturns square root of n")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number
          :tags $ #{} :builtin :internal
        |starts-with? $ %{} :CodeEntry (:doc "|internal function for checking string prefix\nSyntax: (starts-with? s prefix)\nParams: s (string), prefix (string)\nReturns: boolean\nReturns true if string starts with prefix")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'String 'String
          :tags $ #{} :builtin :internal
        |str $ %{} :CodeEntry (:doc "|converts values to string and concatenates them")
          :code $ quote
            defn str (x0 & xs)
              if (&list:empty? xs) (&str x0)
                &str:concat x0 $ str & xs
          :examples $ []
            quote $ assert= |hello (str |hello)
            quote $ assert= |abc (str |a |b |c)
            quote $ assert= |123 (str 1 2 3)
            quote $ assert= "|hello world" (str |hello "| " |world)
          :schema $ :: 'Fn
            {} (:rest 'Dynamic) (:return 'String)
              :args $ [] 'Dynamic
        |str-spaced $ %{} :CodeEntry (:doc "|converts values to string and joins them with spaces")
          :code $ quote
            defn str-spaced (& xs) (&str-spaced true & xs)
          :examples $ []
            quote $ assert= "|a b c" (str-spaced |a |b |c)
            quote $ assert= "|1 2 3" (str-spaced 1 2 3)
          :schema $ :: 'Fn
            {} (:rest 'Dynamic) (:return 'String)
              :args $ []
        |string? $ %{} :CodeEntry (:doc "|checks if value is a string")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ assert= true (string? |hello)
            quote $ assert= false (string? 123)
            quote $ assert= false (string? :keyword)
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |strip-prefix $ %{} :CodeEntry (:doc "|removes prefix from string if it starts with that prefix, returns original string otherwise")
          :code $ quote
            defn strip-prefix (s piece)
              if (starts-with? s piece)
                &str:slice s $ &str:count piece
                , s
          :examples $ []
            quote $ assert= "| world" (strip-prefix "|hello world" |hello)
            quote $ assert= |abc (strip-prefix |prefix-abc |prefix-)
            quote $ assert= |hello (strip-prefix |hello |xyz)
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'String 'String
        |strip-suffix $ %{} :CodeEntry (:doc "|removes suffix from string if it ends with that suffix, returns original string otherwise")
          :code $ quote
            defn strip-suffix (s piece)
              if (ends-with? s piece)
                &str:slice s 0 $ &- (&str:count s) (&str:count piece)
                , s
          :examples $ []
            quote $ assert= |hello (strip-suffix "|hello world" "| world")
            quote $ assert= |abc (strip-suffix |abc-suffix |-suffix)
            quote $ assert= |hello (strip-suffix |hello |xyz)
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'String 'String
        |struct? $ %{} :CodeEntry (:doc "|Predicate that checks whether a value is a struct definition.")
          :code $ quote
            defn struct? (x)
              &= (type-of x) :struct
          :examples $ []
            quote $ assert= true
              struct? $ defstruct Person (:name 'String)
            quote $ assert= false
              struct? $ {} (:x 1)
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Dynamic
        |swap! $ %{} :CodeEntry (:doc "|Atomically updates a reference by applying a function to its current value and storing the result.")
          :code $ quote
            defmacro swap! (a f & args)
              quasiquote $ reset! ~a
                ~f (&atom:deref ~a) ~@args
          :examples $ []
            quote $ do (defatom *counter 0) (swap! *counter inc)
              assert= 1 $ deref *counter
            quote $ do (defatom *state 1) (swap! *state + 2)
              assert= 3 $ deref *state
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic 'Dynamic)
          :tags $ #{} :macro :state
        |symbol? $ %{} :CodeEntry (:doc "|Predicate that checks whether a value is a symbol literal (as opposed to strings, keywords, or other data).")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ assert= true
              symbol? $ quote item
            quote $ assert= false (symbol? |text)
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |syntax? $ %{} :CodeEntry (:doc "|detecting syntax element")
          :code $ quote
            defn syntax? (x)
              &= (type-of x) :syntax
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T
              :generics $ [] 'T
        |tag-match $ %{} :CodeEntry (:doc "|Pattern matching on tagged tuples, dispatches based on the first element of the tuple")
          :code $ quote
            defmacro tag-match (value & body)
              if (&list:empty? body) (raise "|tag-match expected some patterns and matches")
                &let
                  t# $ gensym |tag
                  &let
                    v# $ gensym |v
                    quasiquote $ &let (~v# ~value)
                      if
                        not $ tuple? ~v#
                        raise $ str "|tag-match expected tuple, got" ~v#
                      &let
                        ~t# $ &tuple:nth ~v# 0
                        &tuple:validate-enum ~v# ~t#
                        internal/&tag-match-internal ~v# ~t# $ ~@ body
          :examples $ []
            quote $ assert= 11
              tag-match (:: :ok 1)
                (:ok v) (&+ v 10)
                (:err e) (eprintln e)
            quote $ assert= |got:hello
              tag-match (:: :some |hello)
                (:some x) (str x |:got)
                (:none) |nothing
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |tag? $ %{} :CodeEntry (:doc "|Check if a value is a tag (keyword)")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ assert= true (tag? :keyword)
            quote $ assert= false (tag? |string)
            quote $ assert= false (tag? 123)
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |tagging-edn $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn tagging-edn (data)
              if (list? data) (map data tagging-edn)
                if (map? data)
                  map-kv data $ defn %tagging (k v)
                    []
                      if (string? k) (turn-tag k) k
                      tagging-edn v
                  , data
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic
        |take $ %{} :CodeEntry (:doc "|Take the first n elements from a list")
          :code $ quote
            defn take (xs n)
              if
                >= n $ &list:count xs
                , xs $ slice xs 0 n
          :examples $ []
            quote $ assert= ([] 1 2)
              take ([] 1 2 3 4) 2
            quote $ assert= ([] 1 2 3)
              take ([] 1 2 3) 5
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T) 'Number
              :generics $ [] 'T
              :return $ :: 'List 'T
        |take-last $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn take-last (xs n)
              if
                >= n $ &list:count xs
                , xs $ slice xs
                  - (&list:count xs) n
                  &list:count xs
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'T) 'Number
              :generics $ [] 'T
              :return $ :: 'List 'T
        |thread-as $ %{} :CodeEntry (:doc "|a alias for `->%`")
          :code $ quote
            defmacro thread-as (& xs)
              if (&list:empty? xs) (raise "|thread-as expects at least 1 expression")
              quasiquote $ ->% ~@xs
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :alias :macro
        |thread-first $ %{} :CodeEntry (:doc "|a alias for `->`")
          :code $ quote
            defmacro thread-first (& xs)
              if (&list:empty? xs) (raise "|thread-first expects at least 1 expression")
              quasiquote $ -> ~@xs
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :alias :macro
        |thread-last $ %{} :CodeEntry (:doc "|a alias for `->>`")
          :code $ quote
            defmacro thread-last (& xs)
              if (&list:empty? xs) (raise "|thread-last expects at least 1 expression")
              quasiquote $ ->> ~@xs
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :alias :macro
        |thread-step? $ %{} :CodeEntry (:doc "|Check whether a value is a valid thread-macro step form")
          :code $ quote
            defn thread-step? (x)
              or (symbol? x) (tag? x)
                = (type-of x) :method
                = (type-of x) :fn
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Dynamic
        |to-lispy-string $ %{} :CodeEntry (:doc "|internal function for converting to Lisp string\nSyntax: (to-lispy-string value)\nParams: value (any)\nReturns: string\nConverts value to Lisp-style string representation")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'Dynamic
          :tags $ #{} :builtin :internal
        |to-pairs $ %{} :CodeEntry (:doc "|internal function for converting to pairs\nSyntax: (to-pairs map)\nParams: map (map)\nReturns: set\nConverts map to an unordered set of [key value] pairs")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V)
              :generics $ [] 'K 'V
              :return $ :: 'Set 'Tuple
          :tags $ #{} :builtin :internal
        |trim $ %{} :CodeEntry (:doc "|internal function for trimming strings\nSyntax: (trim s)\nParams: s (string)\nReturns: string\nRemoves whitespace from beginning and end of string")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'String
          :tags $ #{} :builtin :internal
        |try $ %{} :CodeEntry (:doc "|internal syntax for try-catch error handling\nSyntax: (try body (catch error handler))\nParams: body (expression), error (symbol), handler (expression)\nReturns: result of body or handler if error occurs\nProvides exception handling mechanism")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :control :internal :syntax
        |tuple? $ %{} :CodeEntry (:doc "|Predicate that checks whether a value is a tuple literal created with the `::` form.")
          :code $ quote &runtime-implementation
          :examples $ []
            quote $ assert= true
              tuple? $ :: :a :b
            quote $ assert= false
              tuple? $ [] :a :b
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |turn-str $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn turn-str (x) (turn-string x)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'T
              :generics $ [] 'T
        |turn-string $ %{} :CodeEntry (:doc "|internal function for converting to string\nSyntax: (turn-string value)\nParams: value (any)\nReturns: string\nConverts value to string representation")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'String)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |turn-symbol $ %{} :CodeEntry (:doc "|internal function for converting to symbol\nSyntax: (turn-symbol value)\nParams: value (string, tag, or symbol)\nReturns: symbol\nConverts string, tag, or existing symbol to symbol type")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic
          :tags $ #{} :builtin :internal
        |turn-tag $ %{} :CodeEntry (:doc "|internal function for converting to tag\nSyntax: (turn-tag value)\nParams: value (string, symbol, or tag)\nReturns: tag\nConverts string, symbol, or existing tag to tag type")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Tag)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |type-of $ %{} :CodeEntry (:doc "|internal function for getting type of value\nSyntax: (type-of value)\nParams: value (any)\nReturns: tag representing the type\nReturns type tag like :nil, :bool, :number, :string, :list, :map, :set, :fn, etc.")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Tag)
              :args $ [] 'T
              :generics $ [] 'T
          :tags $ #{} :builtin :internal
        |union $ %{} :CodeEntry (:doc "|Returns the union of all sets")
          :code $ quote
            defn union (base & xs)
              reduce xs base $ fn (acc item) (&union acc item)
          :examples $ []
            quote $ assert= (#{} 1 2 3 4)
              union (#{} 1 2) (#{} 3 4)
            quote $ assert= (#{} 1 2 3)
              union (#{} 1) (#{} 2) (#{} 3)
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Set 'T)
              :generics $ [] 'T
              :rest $ :: 'Set 'T
              :return $ :: 'Set 'T
        |unix-time-ms $ %{} :CodeEntry (:doc "|Return the current Unix timestamp in milliseconds.\nSyntax: (unix-time-ms)\nReturns: number")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ []
          :tags $ #{} :builtin :internal :io :time
        |unsafe-coerce $ %{} :CodeEntry (:doc "||Explicitly attach a type annotation without a runtime validation.\\nSyntax: (unsafe-coerce value type-expr)\\nParams: value (any), type-expr (type annotation)\\nReturns: value unchanged\\nUse only at trusted FFI boundaries so downstream code can be statically checked. The declared type is not validated at runtime.")
          :code $ quote (def unsafe-coerce &runtime-implementation)
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal :meta :syntax
        |unselect-keys $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn unselect-keys (m xs)
              assert "|expected map for unselecting" $ map? m
              foldl xs m $ defn %unselect-keys (acc k)
                hint-fn $ {}
                  :args $ [] (:: 'Map 'K 'V) 'K
                  :return $ :: 'Map 'K 'V
                &map:dissoc acc k
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V) (:: 'List 'K)
              :generics $ [] 'K 'V
              :return $ :: 'Map 'K 'V
        |update $ %{} :CodeEntry (:doc "|Applies a function to the value at a given key or index, returning a collection with the updated slot.")
          :code $ quote
            defn update (x k f)
              if (map? x)
                if (contains? x k)
                  assoc x k $ f (&map:get x k)
                  , x
                if (list? x)
                  if (&list:contains? x k)
                    assoc x k $ f (&list:nth x k)
                    , x
                  if (tuple? x)
                    assoc x k $ f (&tuple:nth x k)
                    if (record? x)
                      if (contains? x k)
                        assoc x k $ f (&record:get x k)
                        , x
                      raise $ &str:concat "|Cannot update key on item: " (to-lispy-string x)
          :examples $ []
            quote $ assert=
              {} $ :count 2
              update
                {} $ :count 1
                , :count inc
            quote $ assert= (:: 0 2 2)
              update (:: 0 1 2) 1 inc
            quote $ assert=
              {} $ :count 1
              update
                {} $ :count 1
                , :missing inc
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic 'Dynamic
                :: 'Fn $ {} (:return 'T)
                  :args $ [] 'T
              :generics $ [] 'T
        |update-in $ %{} :CodeEntry (:doc "|Walks a path of keys inside nested maps/lists and applies a function to the value, creating intermediate maps as needed.")
          :code $ quote
            defn update-in (data path f)
              list-match path
                () $ f data
                (p0 ps)
                  assoc
                    either data $ {}
                    , p0 $ update-in (get data p0) ps f
          :examples $ []
            quote $ assert=
              {} $ :a
                {} $ :b 2
              update-in
                {} $ :a
                  {} $ :b 1
                [] :a :b
                , inc
            quote $ assert=
              {} $ :profile
                {} $ :visits 1
              update-in {} ([] :profile :visits)
                fn (_missing) 1
            quote $ assert=
              {} $ :x 10
              update-in
                {} $ :x 5
                [] :x
                fn (v) (&* v 2)
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic (:: 'List 'K)
                :: 'Fn $ {} (:return 'T)
                  :args $ [] 'T
              :generics $ [] 'K 'T
        |vals $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn vals (x)
              map (to-pairs x) last
          :examples $ []
            quote $ assert= (#{} 1 2)
              vals $ {} (:a 1) (:b 2)
            quote $ assert= (#{})
              vals $ {}
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'Map 'K 'V)
              :generics $ [] 'K 'V
              :return $ :: 'Set 'V
        |w-js-log $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro w-js-log (x)
              if (list? x)
                &let
                  v $ if
                    = :eval $ &get-calcit-running-mode
                    gensym |v
                    , '_log_tmp
                  quasiquote $ &let (~v ~x)
                    js/console.log
                      format-to-lisp $ quote ~x
                      , |=> ~v
                    ~ v
                quasiquote $ &let ()
                  js/console.log
                    format-to-lisp $ quote ~x
                    , |=> ~x
                  ~ x
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |w-log $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro w-log (x)
              &let
                v $ if
                  = :eval $ &get-calcit-running-mode
                  gensym |v
                  , '_log_tmp
                if (list? x)
                  quasiquote $ &let (~v ~x)
                    println
                      format-to-lisp $ quote ~x
                      , |=> ~v
                    ~ v
                  quasiquote $ &let ()
                    println
                      format-to-lisp $ quote ~x
                      , |=> ~x
                    ~ x
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |when $ %{} :CodeEntry (:doc "|Conditional macro that evaluates its body only when the test expression is truthy, returning the last body value.")
          :code $ quote
            defmacro when (condition & body)
              if (&list:empty? body) (raise "|when expects at least 1 body expression")
              if
                &= 1 $ &list:count body
                quasiquote $ if ~condition
                  ~ $ nth body 0
                quasiquote $ if ~condition
                  &let () ~@body
          :examples $ []
            quote $ assert= 4
              when (&> 3 2) (inc 3)
            quote $ assert= nil
              when false $ inc 1
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |when-let $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro when-let (pair & body)
              if
                not $ and (list? pair)
                  &= 2 $ count pair
                raise $ str-spaced "|expected a pair, got:" pair
              &let
                x $ nth pair 0
                if
                  not $ symbol? x
                  raise $ str-spaced "|expected a symbol for var name, got:" x
                quasiquote $ &let
                  ~x $ ~ (nth pair 1)
                  if (some? ~x)
                    do $ ~@ body
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |when-not $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro when-not (condition & body)
              if (&list:empty? body) (raise "|when-not expects at least 1 body expression")
              if
                &= 1 $ &list:count body
                quasiquote $ if (not ~condition)
                  ~ $ nth body 0
                quasiquote $ if (not ~condition)
                  &let () ~@body
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |with-cpu-time $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro with-cpu-time (x)
              let
                  started $ gensym |started
                  v $ gensym |v
                quasiquote $ let
                    ~started $ cpu-time
                    ~v ~x
                  println |[cpu-time]
                    format-to-lisp $ quote ~x
                    , |=>
                      .format
                        &- (cpu-time) ~started
                        , 3
                      , |ms
                  ~ v
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :io :log :macro
        |with-gensyms $ %{} :CodeEntry (:doc "|Macro helper for hygienic local names\nSyntax: (with-gensyms (a b ...) body...)\nBinds each symbol to a fresh gensym and evaluates body with those bindings.")
          :code $ quote
            defmacro with-gensyms (names & body)
              assert "|with-gensyms expects a list of symbols" $ and (list? names) (every? names symbol?)
              reduce (reverse names)
                quasiquote $ do (~@ body)
                fn (acc n)
                  quasiquote $ let
                      ~n $ gensym
                    , ~acc
          :examples $ []
            quote $ with-gensyms (v)
              quasiquote $ &let (~v 1) ~v
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |with-type-slot $ %{} :CodeEntry (:doc "|Compatibility form for a local compile-time type-slot override. Syntax: (with-type-slot (:slot-name TypeExpr) body...). The form is always erased during preprocessing; it evaluates bodies in order and returns the last value. New entry points should prefer :type-slots configuration.")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic
          :tags $ #{} :builtin :internal :meta :state :syntax
        |wo-js-log $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro wo-js-log (x) x
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |wo-log $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro wo-log (x) x
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
          :tags $ #{} :macro
        |write-file $ %{} :CodeEntry (:doc "|internal function for writing files\nSyntax: (write-file filepath content)\nParams: filepath (string), content (string)\nReturns: nil or error\nWrites string content to file, creates directories if needed")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ [] 'String 'String
          :tags $ #{} :builtin :file :internal :io
        |zipmap $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn zipmap (xs0 ys0)
              apply-args
                  {}
                  , xs0 ys0
                fn (acc xs ys)
                  if
                    if (&list:empty? xs) true $ &list:empty? ys
                    , acc $ recur
                      &map:assoc acc (&list:first xs) (&list:first ys)
                      rest xs
                      rest ys
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] (:: 'List 'K) (:: 'List 'V)
              :generics $ [] 'K 'V
              :return $ :: 'Map 'K 'V
        |{,} $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro {,} (& body)
              &let
                xs $ &list:filter body
                  defn &{,} (x)
                    hint-fn $ {} (:return 'Bool)
                    not= x ',
                quasiquote $ pairs-map
                  section-by ([] ~@xs) 2
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :macro
        |{} $ %{} :CodeEntry (:doc "|macro for creating hashmaps\nSyntax: ({} (:key value) ...)\nParams: pairs (key-value pairs)\nReturns: hashmap\nCreates a hashmap from key-value pairs")
          :code $ quote
            defmacro {} (& xs)
              if
                not $ every? xs
                  fn (pair)
                    and (list? pair)
                      &= 2 $ &list:count pair
                raise $ str "|{} expects pairs of lists with exactly two elements each, got: " xs
              &let
                ys $ &list:concat & xs
                quasiquote $ &{} ~@ys
          :examples $ []
            quote $ {} (:a 1) (:b 2)
            quote $ {} (:name |Alice) (:age 30)
          :schema $ :: 'Macro
            {} $ :args ([])
          :tags $ #{} :macro
        |~ $ %{} :CodeEntry (:doc "|internal syntax for interpolating value in macro\nSyntax: (~ expr) inside quasiquote\nParams: expr (expression to evaluate)\nReturns: evaluated expression\nUnquotes expression inside quasiquote")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal :syntax
        |~@ $ %{} :CodeEntry (:doc "|internal syntax for spreading interpolate value in macro\nSyntax: (~@ list-expr) inside quasiquote\nParams: list-expr (expression that evaluates to list)\nReturns: spliced list elements\nUnquotes and splices list elements inside quasiquote")
          :code $ quote &runtime-implementation
          :examples $ []
          :schema $ :: 'Dynamic
          :tags $ #{} :builtin :internal :syntax
      :ns $ %{} :NsEntry (:doc "|built-in function and macros in `calcit.core`")
        :code $ quote
          ns calcit.core $ :require (calcit.internal :as internal)
    |calcit.internal $ %{} :FileEntry
      :defs $ {}
        |&core-add-list-impl $ %{} :CodeEntry (:doc "|Core trait impl for Add on list")
          :code $ quote
            def &core-add-list-impl $ &impl::new :&core-add-list-impl (:: :add &list:concat)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-add-number-impl $ %{} :CodeEntry (:doc "|Core trait impl for Add on number")
          :code $ quote
            def &core-add-number-impl $ &impl::new :&core-add-number-impl (:: :add &+)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-add-string-impl $ %{} :CodeEntry (:doc "|Core trait impl for Add on string")
          :code $ quote
            def &core-add-string-impl $ &impl::new :&core-add-string-impl (:: :add &str:concat)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-compare-number-impl $ %{} :CodeEntry (:doc |)
          :code $ quote
            def &core-compare-number-impl $ &impl::new :&core-compare-number-impl (:: :compare &compare)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-compare-string-impl $ %{} :CodeEntry (:doc |)
          :code $ quote
            def &core-compare-string-impl $ &impl::new :&core-compare-string-impl (:: :compare &str:compare)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-contains-list-impl $ %{} :CodeEntry (:doc |)
          :code $ quote
            def &core-contains-list-impl $ &impl::new :&core-contains-list-impl (:: :contains? &list:contains?)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-contains-map-impl $ %{} :CodeEntry (:doc |)
          :code $ quote
            def &core-contains-map-impl $ &impl::new :&core-contains-map-impl (:: :contains? &map:contains?)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-contains-record-impl $ %{} :CodeEntry (:doc |)
          :code $ quote
            def &core-contains-record-impl $ &impl::new :&core-contains-record-impl (:: :contains? &record:contains?)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-contains-set-impl $ %{} :CodeEntry (:doc |)
          :code $ quote
            def &core-contains-set-impl $ &impl::new :&core-contains-set-impl (:: :contains? &set:includes?)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-contains-string-impl $ %{} :CodeEntry (:doc |)
          :code $ quote
            def &core-contains-string-impl $ &impl::new :&core-contains-string-impl (:: :contains? &str:contains?)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-contains-tuple-impl $ %{} :CodeEntry (:doc |)
          :code $ quote
            def &core-contains-tuple-impl $ &impl::new :&core-contains-tuple-impl
              :: :contains? $ fn (x k)
                if (&>= k 0)
                  &< k $ &tuple:count x
                  , false
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-countable-list-impl $ %{} :CodeEntry (:doc |)
          :code $ quote
            def &core-countable-list-impl $ &impl::new :&core-countable-list-impl (:: :count &list:count)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-countable-map-impl $ %{} :CodeEntry (:doc |)
          :code $ quote
            def &core-countable-map-impl $ &impl::new :&core-countable-map-impl (:: :count &map:count)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-countable-record-impl $ %{} :CodeEntry (:doc |)
          :code $ quote
            def &core-countable-record-impl $ &impl::new :&core-countable-record-impl (:: :count &record:count)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-countable-set-impl $ %{} :CodeEntry (:doc |)
          :code $ quote
            def &core-countable-set-impl $ &impl::new :&core-countable-set-impl (:: :count &set:count)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-countable-string-impl $ %{} :CodeEntry (:doc |)
          :code $ quote
            def &core-countable-string-impl $ &impl::new :&core-countable-string-impl (:: :count &str:count)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-countable-tuple-impl $ %{} :CodeEntry (:doc |)
          :code $ quote
            def &core-countable-tuple-impl $ &impl::new :&core-countable-tuple-impl (:: :count &tuple:count)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-eq-impl $ %{} :CodeEntry (:doc "|Core trait impl for Eq")
          :code $ quote
            def &core-eq-impl $ &impl::new :&core-eq-impl (:: :eq? &=)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-len-list-impl $ %{} :CodeEntry (:doc "|Core trait impl for Len on list")
          :code $ quote
            def &core-len-list-impl $ &impl::new :&core-len-list-impl (:: :len &list:count)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-len-map-impl $ %{} :CodeEntry (:doc "|Core trait impl for Len on map")
          :code $ quote
            def &core-len-map-impl $ &impl::new :&core-len-map-impl (:: :len &map:count)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-len-set-impl $ %{} :CodeEntry (:doc "|Core trait impl for Len on set")
          :code $ quote
            def &core-len-set-impl $ &impl::new :&core-len-set-impl (:: :len &set:count)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-len-string-impl $ %{} :CodeEntry (:doc "|Core trait impl for Len on string")
          :code $ quote
            def &core-len-string-impl $ &impl::new :&core-len-string-impl (:: :len &str:count)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-mappable-list-impl $ %{} :CodeEntry (:doc "|Core trait impl for Mappable on list")
          :code $ quote
            def &core-mappable-list-impl $ &impl::new :&core-mappable-list-impl (:: :map &list:map)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-mappable-map-impl $ %{} :CodeEntry (:doc "|Core trait impl for Mappable on map")
          :code $ quote
            def &core-mappable-map-impl $ &impl::new :&core-mappable-map-impl (:: :map &map:map)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-mappable-set-impl $ %{} :CodeEntry (:doc |)
          :code $ quote
            def &core-mappable-set-impl $ &impl::new :&core-mappable-set-impl (:: :map &set:map)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-multiply-number-impl $ %{} :CodeEntry (:doc "|Core trait impl for Multiply on number")
          :code $ quote
            def &core-multiply-number-impl $ &impl::new :&core-multiply-number-impl (:: :multiply &*)
          :examples $ []
          :schema $ :: 'Dynamic
        |&core-show-impl $ %{} :CodeEntry (:doc "|Core trait impl for Show")
          :code $ quote
            def &core-show-impl $ &impl::new :&core-show-impl (:: :show &str)
          :examples $ []
          :schema $ :: 'Dynamic
        |&field-match-internal $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro &field-match-internal (value & body)
              if (&list:empty? body)
                quasiquote $ eprintln "|[Warn] field-match found no matched case, missing `_` case?" ~value
                &let
                  pair $ first body
                  if
                    not $ list? pair
                    raise $ str-spaced "|field-match expected arm in list, got:" pair
                  let
                      pattern $ &list:nth pair 0
                    assert "|expected literal or symbol as tag" $ or (tag? pattern) (symbol? pattern)
                    if (&= pattern '_)
                      &let ()
                        assert "|field-match expected a branch after `_`" $ &= 2 (&list:count pair)
                        if
                          not $ &= 1 (&list:count body)
                          eprintln "|[Warn] expected `_` beginning last branch of field-match"
                        &list:nth pair 1
                      &let ()
                        assert "|field-match expected an with (tag new-name body)" $ &= 3 (&list:count pair)
                        quasiquote $ if
                          &= ~pattern $ &map:get ~value :tag
                          &let
                              ~ $ &list:nth pair 1
                              , ~value
                            ~ $ &list:nth pair 2
                          &field-match-internal ~value $ ~@ (&list:rest body)
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic)
        |&tag-match-internal $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro &tag-match-internal (value t & body)
              if (&list:empty? body)
                quasiquote $ raise (str-spaced "|tag-match found no matched case, missing `_` for" ~value)
                &let
                  pair $ &list:first body
                  if
                    not $ and (list? pair)
                      &= 2 $ &list:count pair
                    raise $ str-spaced "|tag-match expected pairs, got:" pair
                  let
                      pattern $ &list:nth pair 0
                      branch $ &list:nth pair 1
                    if (list? pattern)
                      &let
                        k $ &list:first pattern
                        &let
                          size $ &list:count pattern
                          quasiquote $ if
                            if (identical? ~t ~k)
                              identical? ~size $ &tuple:count ~value
                              , false
                            let
                              ~ $ map-indexed (&list:rest pattern)
                                defn %tag-match (idx x)
                                  [] x $ quasiquote
                                    &tuple:nth ~value $ ~ (inc idx)
                              , ~branch
                            &tag-match-internal ~value ~t $ ~@ (&list:rest body)
                      if (&= pattern '_) branch $ raise (str-spaced "|unknown supported pattern:" pair)
          :examples $ []
          :schema $ :: 'Macro
            {} $ :args ([] 'Dynamic 'Dynamic)
        |normalize-trait-type $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn normalize-trait-type (t0)
              if (list? t0)
                &let
                  size $ &list:count t0
                  &let
                    head $ &list:first t0
                    &let
                      second $ if (&>= size 2) (&list:nth t0 1) nil
                      &let
                        third $ if (&>= size 3) (&list:nth t0 2) nil
                        &let
                          wrap-list $ fn (x)
                            if (list? x) x $ [] x
                          if
                            and (tag? head) (&= head :fn) (&= size 3)
                            [] :fn ([]) (wrap-list second) (&list:nth t0 2)
                            if
                              and (tag? head) (&= head :fn) (&= size 4)
                              [] :fn (wrap-list second) (wrap-list third) (&list:nth t0 3)
                              if
                                and (tag? second) (&= second :fn) (&= size 4)
                                [] head second ([]) (wrap-list third) (&list:nth t0 3)
                                if
                                  and (tag? second) (&= second :fn) (&= size 5)
                                  [] head second (wrap-list third)
                                    wrap-list $ &list:nth t0 3
                                    &list:nth t0 4
                                  , t0
                , t0
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} :NsEntry (:doc "|internal function and macros for `calcit.core`")
        :code $ quote
          ns calcit.internal $ :require
