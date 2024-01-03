use anyhow::Result;
use derive_builder::Builder;


use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tempfile::tempdir;
use tempfile::NamedTempFile;

#[derive(Builder, Clone)]
pub struct Caller {
    genome: PathBuf,
    reads: Vec<PathBuf>,
    // output: Option<PathBuf>,
}

impl Caller {
    pub fn call(&self) -> Result<()> {
        //1-) index the linear genome
        //bwa index -p hs_genome -a bwtsw hs_genome.fasta

        //todo: uncomment the indexing part and consider caching.

        //create a temporary file for bwa index and execute bwa index
        // let temp_index = NamedTempFile::new()?;
        // let index = {
        //     Command::new("bwa")
        //         .arg("index")
        //         .arg("-p")
        //         .arg(temp_index.path())
        //         .arg("-a")
        //         .arg("bwtsw") //-a bwtsw' does not work for short genomes, lineage quantification?
        //         .arg(self.genome.clone())
        //         .status()
        //         .expect("failed to execute indexing process")
        // };
        // println!("The index was created successfully: {}", index);

        // configure thread_number
        let thread_number = "10".to_string();

        //perform the alignment for paired end reads
        let _temp_aligned = NamedTempFile::new()?;

        // Create a directory inside of `std::env::temp_dir()`
        let temp_dir = tempdir()?;

        //find sample name of one of the fastq files from the read pair
        let stem_of_sample_dir = &self.reads[0].file_stem().unwrap().to_str().unwrap();
        //get rid of any underscore (PE reads contain them)
        let splitted = stem_of_sample_dir.split('_').collect::<Vec<&str>>();
        let sample_name = splitted[0];

        //create the output file name in temp directory
        let file_aligned = temp_dir.path().join(format!("{}.bam", sample_name));
        println!("{}", file_aligned.display());

        //insert read_group info from the sample names
        let read_group = format!("@RG\\tID:{}\\tSM:{}", sample_name, sample_name);

        //test the subcommand, for this skip the indexing part
        let index = "/projects/koesterlab/orthanq/orthanq-evaluation/results/bwa-index/hs_genome";

        //Step-1: align reads to the bwa index
        let align = {
            Command::new("bwa")
                .arg("mem")
                .arg("-t")
                .arg("10")
                .arg("-R")
                .arg(&read_group)
                // .arg(&temp_index.path())
                .arg(index)
                .arg(&self.reads[0])
                .arg(&self.reads[1])
                .arg("-o")
                .arg(&file_aligned)
                // .arg("2>")
                // .arg("log.txt")
                .status()
                .expect("failed to execute the alignment process")
        };
        println!("The alignment was exited with: {}", align);
        println!("{}", file_aligned.display());
        //sort the aligned reads by coordinate

        //create the output file name in temp directory
        let file_aligned_sorted = temp_dir.path().join(format!("{}_sorted.bam", sample_name));

        let sort = {
            Command::new("samtools")
                .arg("sort")
                .arg(file_aligned)
                .arg("-o")
                .arg(&file_aligned_sorted)
                .arg("-@")
                .arg(&thread_number)
                .arg("--write-index")
                .status()
                .expect("failed to execute the sorting process")
        };
        println!("The sorting was exited with: {}", sort);
        println!("{}", file_aligned_sorted.display());

        //Step-2: extract reads that map to HLA genes (classical and nonclassical class of genes)

        //create the output file name in temp directory
        let file_extracted = temp_dir
            .path()
            .join(format!("{}_extracted.bam", sample_name));

        let regions = "resources/regions.bed";

        let extract = {
            Command::new("samtools")
                .arg("view")
                .arg(file_aligned_sorted)
                .arg("-L")
                .arg(regions)
                .arg("--write-index") //??
                .arg("-o")
                .arg(&file_extracted)
                .status()
                .expect("failed to execute the extracting process")
        };
        println!("The extraction was exited with: {}", extract);

        //convert the alignment file to fq

        //create the output file name in temp directory
        let temp_extracted_fq_1 = temp_dir.path().join(format!("{}_1.fastq", sample_name));
        let temp_extracted_fq_2 = temp_dir.path().join(format!("{}_2.fastq", sample_name));

        let bam_to_fq = {
            Command::new("samtools")
                .arg("fastq")
                .arg(file_extracted)
                .arg("-n") //-n for fastq
                .arg("-1")
                .arg(&temp_extracted_fq_1)
                .arg("-2")
                .arg(&temp_extracted_fq_2)
                .status()
                .expect("failed to execute the extracting process")
        };
        println!("Conversion from BAM to fq was exited with: {}", bam_to_fq);

        //Step-3: map extracted reads to the pangenome with vg giraffe

        //path to the index directory
        let giraffe_index = "resources/hprc-v1.0-mc-grch38.xg";

        //create the output file name in temp directory
        let file_aligned_pangenome = temp_dir
            .path()
            .join(format!("{}_vg.bam", sample_name));

        let align_pangenome = {
            Command::new("vg")
                .arg("giraffe")
                .arg("-x")
                .arg(giraffe_index)
                .arg("-f")
                .arg(temp_extracted_fq_1)
                .arg("-f")
                .arg(temp_extracted_fq_2)
                .arg("--output-format")
                .arg("BAM")
                .arg("-t")
                .arg(&thread_number)
                .stdout(Stdio::piped())
                .spawn()
                .expect("failed to execute the vg giraffe process")
        };
        println!(
            "Alignment to pangenome was exited with: {:?}",
            align_pangenome
        );

        //write bam to file (buffered)
        // let mut vg_bam = std::fs::File::create(&file_aligned_pangenome)?;
        // let mut f = std::fs::File::open(&file_aligned_pangenome).unwrap();
        // let mut f = std::io::BufWriter::new(f);
        // {
        //     let stdout = align_pangenome.stdout;
        //     let stdout_reader = std::io::BufReader::new(stdout);
        //     let stdout_lines = stdout_reader.bytes();

        //     for line in stdout_lines {
        //         f.write(&[line.unwrap()]);
        //     }
        // }
        // align_pangenome.wait().unwrap();
        // f.flush()?;

        // let output_align = align_pangenome.stdout.expect("failed to wait on aligning to pangenome");

        let output = align_pangenome
            .wait_with_output()
            .expect("Failed to read stdout");

        let mut vg_bam = std::fs::File::create(file_aligned_pangenome.clone())?;
        vg_bam.write_all(&output.stdout)?; //write with bam writer
        vg_bam.flush()?;

        //sort the resulting vg aligned file
        let file_vg_aligned_sorted = temp_dir
            .path()
            .join(format!("{}_vg_sorted.bam", sample_name));

        let vg_sort = {
            Command::new("samtools")
                .arg("sort")
                .arg(&file_aligned_pangenome)
                .arg("-o")
                .arg(&file_vg_aligned_sorted)
                .arg("-@")
                .arg(&thread_number)
                .arg("--write-index")
                .status()
                .expect("failed to execute the sorting process")
        };
        println!("The sorting was exited with: {}", vg_sort);
        println!("{}", file_vg_aligned_sorted.display());

        //modify the header for chromosome names to be compatible with the reference genome that we acquire from ensembl

        //prepare the temporary file path for the reheadered bam output
        let file_reheadered = temp_dir
            .path()
            .join(format!("{}_reheadered.bam", sample_name));

        println!("{}", file_reheadered.display());

        //in Rust, piping cannot be done via "|" but instead in the following way:

        //get the header
        let samtools_view_child = Command::new("samtools")
            .arg("view") // `samtools view` command...
            .arg("-H") // of which we will pipe the output.
            .arg(&file_vg_aligned_sorted) //Once configured, we actually spawn the command...
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        //replace the 'GRCh38.chr' with ''
        let sed_child_one = Command::new("sed")
            .arg("s/GRCh38.chr//g")
            .stdin(Stdio::from(samtools_view_child.stdout.unwrap())) // Pipe through.
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        //then, reheader the header of the input bam
        let reheader_child_two = Command::new("samtools")
            .arg("reheader")
            .arg("-")
            .stdin(sed_child_one.stdout.unwrap())
            .arg(file_vg_aligned_sorted)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        //write the reheadered bam to file
        let output = reheader_child_two
            .wait_with_output()
            .expect("failed to wait on child");
        let mut f = std::fs::File::create(file_reheadered.clone())?;
        f.write_all(&output.stdout)?;

        //index the resulting bam file
        let samtools_index = {
            Command::new("samtools")
                .arg("index")
                .arg(&file_reheadered)
                .status()
                .unwrap()
        };

        println!("The indexing was exited with: {}", samtools_index);

        //finally, extract only strandard chromosomes
        let parent = &self.reads[0].parent().unwrap();
        let final_bam = parent.join(format!("{}_processed.bam", sample_name));
        println!("{}", final_bam.display());

        let samtools_extract = {
            Command::new("samtools")
                .arg("view")
                .arg(&file_reheadered)
                .args([
                    "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14",
                    "15", "16", "17", "18", "19", "20", "21", "22", "X", "Y", "M",
                ])
                .arg("-o")
                .arg(&final_bam)
                .arg("-@")
                .arg(&thread_number)
                .arg("--write-index")
                .status()
                .expect("failed to execute the sorting process")
        };

        //write the final bam to file
        println!(
            "The extractiong of standard chromosomes was exited with: {}",
            samtools_extract
        );

        // close the file handle of the named temporary files
        // temp_index.close()?;
        temp_dir.close()?;

        Ok(())
    }
}
